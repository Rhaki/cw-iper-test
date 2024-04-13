use crate::{error::AppResult, ibc::IbcChannelCreator, ibc_app::IbcAppRef, response::RelayedResponse};
use anyhow::anyhow;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

#[derive(Default)]
pub struct Ecosystem {
    apps: BTreeMap<String, Rc<RefCell<dyn IbcAppRef>>>,
}

impl Ecosystem {
    pub fn add_app(self, app: Rc<RefCell<dyn IbcAppRef>>) -> Self {
        let mut apps = self.apps;
        let chain_id = app.borrow().chain_id().to_string();
        apps.insert(chain_id, app);
        Self { apps }
    }

    pub fn open_ibc_channel(
        &self,
        mut channel_1: IbcChannelCreator,
        mut channel_2: IbcChannelCreator,
    ) -> AppResult<()> {
        let app_1 = self.get_app(&channel_1.chain_id)?;
        let app_2 = self.get_app(&channel_2.chain_id)?;

        let channel_id_1 = app_1.borrow().get_next_channel_id();
        let channel_id_2 = app_2.borrow().get_next_channel_id();
        channel_1.set_channel_id(channel_id_1);
        channel_2.set_channel_id(channel_id_2);

        let sequence = Rc::new(RefCell::new(0));

        app_1
            .borrow_mut()
            .open_channel(&channel_1, &channel_2, sequence.clone())?;
        app_2
            .borrow_mut()
            .open_channel(&channel_2, &channel_1, sequence)?;

        app_1.borrow_mut().channel_connect(channel_id_1)?;
        app_2.borrow_mut().channel_connect(channel_id_2)?;

        app_1.borrow_mut().channel_connect(channel_id_1)?;
        app_2.borrow_mut().channel_connect(channel_id_2)?;

        Ok(())
    }

    pub fn relay_all_packets(&self) -> AppResult<Vec<RelayedResponse>> {
        let mut res = vec![];

        let mut finished = false;

        while !finished {
            finished = true;

            for (chain_id, app) in &self.apps {
                if app.borrow().some_pending_packets() {
                    res.push(self.relay_next_packet(chain_id)?);
                    finished = false;
                    break;
                }
            }
        }

        Ok(res)
    }

    pub fn relay_next_packet(
        &self,
        chain_id: impl Into<String> + Clone,
    ) -> AppResult<RelayedResponse> {
        let app = self.get_app(chain_id.clone())?;
        let packet_id = app.borrow().get_next_pending_packet()?;
        self.relay_packet(chain_id, packet_id)
    }

    pub fn relay_packet(
        &self,
        chain_id: impl Into<String>,
        packet_id: u64,
    ) -> AppResult<RelayedResponse> {
        let app_src = self.get_app(chain_id)?;

        let msg = app_src.borrow().get_pending_packet(packet_id)?;

        let dest_channel = app_src.borrow().get_dest_channel_from_msg(&msg)?;

        let app_dest = self.get_app(&dest_channel.chain_id)?;

        let dest_response = app_dest
            .borrow_mut()
            .packet_receive(&msg, dest_channel.channel_id()?)?;

        let ack_response = if let Some(ack) = &dest_response.ack {
            Some(app_src.borrow_mut().packet_ack(ack.clone(), &msg)?)
        } else {
            None
        };

        app_src.borrow_mut().remove_packet(packet_id)?;

        Ok(RelayedResponse {
            msg,
            dest_response: dest_response.response,
            ack: dest_response.ack,
            src_response: ack_response,
        })
    }

    fn get_app(&self, chain_id: impl Into<String>) -> AppResult<&Rc<RefCell<dyn IbcAppRef>>> {
        let chain_id: String = chain_id.into();
        self.apps
            .get(&chain_id)
            .ok_or(anyhow!("App not found for chain_id: {}", chain_id))
    }
}
