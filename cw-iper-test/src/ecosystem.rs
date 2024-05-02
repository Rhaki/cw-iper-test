use crate::{
    error::AppResult,
    ibc::IbcChannelCreator,
    ibc_module::IbcPacketType,
    iper_app::{IperAppRef, MayResponse},
};
use anyhow::anyhow;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

#[derive(Default)]
/// This structure acts as a wrapper containing all [`IperApp`](crate::iper_app::IperApp).
///
/// Its primary purpose is to relay `IBC` `packets` and to facilitate the creation of `IBC` `channels` among various [`IperApp`](crate::iper_app::IperApp).
///
/// [`IperApp`](crate::iper_app::IperApp)s are stored as [`IperAppRef`] traits instead of as [`IperApp`](crate::iper_app::IperApp) instances.
/// This approach is used to decouple them from the specific typing of the generic parameters
/// required by the [`IperApp`](crate::iper_app::IperApp) and [`App`](cw_multi_test::App) classes.
pub struct Ecosystem {
    apps: BTreeMap<String, Rc<RefCell<dyn IperAppRef>>>,
}

impl Ecosystem {
    /// Add a [`IperApp`](crate::iper_app::IperApp) as [`IperAppRef`]
    pub fn add_app(self, app: Rc<RefCell<dyn IperAppRef>>) -> Self {
        let mut apps = self.apps;
        let chain_id = app.borrow().chain_id().to_string();
        apps.insert(chain_id, app);
        Self { apps }
    }

    /// Open a `IbcChannel` bewteen two [`IperApp`](crate::iper_app::IperApp)
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

    /// Relay all `packets` untill not `packets` are in pending.
    /// The order is based on the [`BTreeMap`] key orders.
    /// Iterating all [`IperApp`](crate::iper_app::IperApp), if one [`IperApp`](crate::iper_app::IperApp) has not pending packets, next [`IperApp`](crate::iper_app::IperApp) is checked.
    /// Once one `packet` is `relayed`, the loop is restarted from the first [`IperApp`](crate::iper_app::IperApp)
    pub fn relay_all_packets(&self) -> AppResult<Vec<MayResponse>> {
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

    /// Relay the next `packet` of a specific [`IperApp`](crate::iper_app::IperApp)
    pub fn relay_next_packet(&self, chain_id: impl Into<String> + Clone) -> AppResult<MayResponse> {
        let app = self.get_app(chain_id.clone())?;
        let packet_id = app.borrow().get_next_pending_packet()?;
        self.relay_packet(chain_id, packet_id)
    }

    /// Relay as specific `packet` of a specific [`IperApp`](crate::iper_app::IperApp)
    pub fn relay_packet(
        &self,
        chain_id: impl Into<String>,
        packet_id: u64,
    ) -> AppResult<MayResponse> {
        let app_src = self.get_app(chain_id)?;

        let packet = app_src.borrow().get_pending_packet(packet_id)?;

        let channel_info = app_src
            .borrow()
            .get_channel_info(packet.get_local_channel_id())?;

        let app_dest = self.get_app(&channel_info.remote.chain_id)?;

        let response = app_dest.borrow_mut().incoming_packet(packet)?;

        app_src.borrow_mut().remove_packet(packet_id)?;

        Ok(response)
    }

    /// Return all pending `packets` between all [`IperApp`](crate::iper_app::IperApp)
    pub fn get_all_pending_packets(
        &self,
    ) -> AppResult<BTreeMap<String, BTreeMap<u64, IbcPacketType>>> {
        let mut map = BTreeMap::new();

        for (chain_id, app) in &self.apps {
            map.insert(chain_id.clone(), app.borrow().get_pending_packets()?);
        }

        Ok(map)
    }

    fn get_app(&self, chain_id: impl Into<String>) -> AppResult<&Rc<RefCell<dyn IperAppRef>>> {
        let chain_id: String = chain_id.into();
        self.apps
            .get(&chain_id)
            .ok_or(anyhow!("App not found for chain_id: {}", chain_id))
    }
}
