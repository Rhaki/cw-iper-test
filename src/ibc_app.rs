use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use anyhow::{anyhow, bail};
use cosmwasm_std::{
    Addr, Api, Binary, CustomMsg, CustomQuery, IbcAcknowledgement, IbcChannel,
    IbcChannelConnectMsg, IbcChannelOpenMsg, IbcMsg, IbcPacket, IbcPacketAckMsg,
    IbcPacketReceiveMsg, Storage,
};
use cw_multi_test::{
    App, AppResponse, Bank, Distribution, Gov, Ibc, Module, Staking, Stargate, Wasm,
};
use serde::de::DeserializeOwned;

use crate::{
    contracts::IbcContract,
    error::AppResult,
    ibc_module::{
        IbcChannelCreator, IbcChannelDebuilder, IbcChannelExt, IbcChannelStatus, IbcChannelWrapper,
        IbcMsgExt, IbcPort, PENDING_PACKETS,
    },
    response::IntoResponse,
};

pub struct IbcApp<BankT, ApiT, StorageT, CustomT, WasmT, StakingT, DistrT, IbcT, GovT, StargateT>
where
    CustomT: Module,
{
    pub app: App<BankT, ApiT, StorageT, CustomT, WasmT, StakingT, DistrT, IbcT, GovT, StargateT>,
    pub code_ids: BTreeMap<u64, Box<dyn IbcContract<CustomT::ExecT, CustomT::QueryT>>>,
    pub channels: BTreeMap<u64, IbcChannelWrapper>,
}

impl<BankT, ApiT, StorageT, CustomT, WasmT, StakingT, DistrT, IbcT, GovT, StargateT>
    IbcApp<BankT, ApiT, StorageT, CustomT, WasmT, StakingT, DistrT, IbcT, GovT, StargateT>
where
    BankT: Bank,
    ApiT: Api,
    StorageT: Storage,
    CustomT: Module,
    WasmT: Wasm<CustomT::ExecT, CustomT::QueryT>,
    StakingT: Staking,
    DistrT: Distribution,
    IbcT: Ibc,
    GovT: Gov,
    StargateT: Stargate,
    CustomT::ExecT: CustomMsg + DeserializeOwned + 'static,
    CustomT::QueryT: CustomQuery + DeserializeOwned + 'static,
{
    pub fn get_pending_packet(&self, packet_id: u64) -> AppResult<IbcMsg> {
        let packets = PENDING_PACKETS.load(self.app.storage())?;
        packets
            .get(&packet_id)
            .cloned()
            .ok_or(anyhow!("Packet not found"))
    }

    pub fn get_next_pending_packet(&self) -> AppResult<u64> {
        let packets = PENDING_PACKETS.load(self.app.storage())?;
        packets
            .first_key_value()
            .map(|(k, _)| k.clone())
            .ok_or(anyhow!("No pending packets"))
    }

    pub fn some_pending_packets(&self) -> bool {
        PENDING_PACKETS
            .load(self.app.storage())
            .map(|val| !val.is_empty())
            .unwrap_or(false)
    }

    pub fn remove_packet(&mut self, packet_id: u64) -> AppResult<()> {
        let mut packets = PENDING_PACKETS.load(self.app.storage())?;
        packets.remove(&packet_id);
        PENDING_PACKETS.save(self.app.storage_mut(), &packets)?;
        Ok(())
    }

    pub fn get_next_channel_id(&self) -> u64 {
        self.channels
            .last_key_value()
            .map(|(k, _)| k + 1)
            .unwrap_or(0)
    }

    pub fn channel_open(
        &mut self,
        local: &IbcChannelCreator,
        remote: &IbcChannelCreator,
        sequence: Rc<RefCell<u64>>,
    ) -> AppResult<IbcChannelWrapper> {
        let channel_wrapper = IbcChannelWrapper::new(local.clone(), remote.clone(), sequence);

        match &local.port {
            IbcPort::Contract(contract) => {
                let code_id = self.app.contract_data(&contract)?.code_id;
                let ibc_details = self
                    .code_ids
                    .get(&code_id)
                    .ok_or(anyhow!("Code ID not found"))?;
                let msg =
                    IbcChannelOpenMsg::new_init(IbcChannel::new_from_creators(local, remote)?);
                self.app.use_contract(&contract, |deps, env| {
                    ibc_details
                        .ibc_channel_open(deps, env, msg)
                        .into_app_response()
                })?;
            }
            IbcPort::Module(_) => todo!(),
        }

        self.channels
            .insert(channel_wrapper.local.channel_id()?, channel_wrapper.clone());

        Ok(channel_wrapper)
    }

    pub fn channel_connect(&mut self, channel_id: u64) -> AppResult<()> {
        let channel = self
            .channels
            .get_mut(&channel_id)
            .ok_or(anyhow!("Channel not found: {}", channel_id))?;
        let msg = match channel.status {
            IbcChannelStatus::Created => IbcChannelConnectMsg::new_ack(
                IbcChannel::new_from_creators(&channel.local, &channel.remote)?,
                channel.remote.version.clone(),
            ),
            IbcChannelStatus::Opening => IbcChannelConnectMsg::new_confirm(
                IbcChannel::new_from_creators(&channel.local, &channel.remote)?,
            ),
            _ => bail!("Invalid channel status"),
        };

        match &channel.local.port {
            IbcPort::Contract(contract) => {
                let code_id = self.app.contract_data(&contract)?.code_id;
                let ibc_details = self
                    .code_ids
                    .get(&code_id)
                    .ok_or(anyhow!("Code ID not found"))?;

                self.app.use_contract(&contract, |deps, env| {
                    ibc_details
                        .ibc_channel_connect(deps, env, msg)
                        .into_app_response()
                })?;
            }
            IbcPort::Module(_) => todo!(),
        }

        channel.status.next()?;

        Ok(())
    }

    pub fn packet_receive(
        &mut self,
        msg: &IbcMsg,
        relayer: &Addr,
        dest_channel_id: u64,
    ) -> AppResult<PacketReceiveResponse> {
        let channel = self
            .channels
            .get_mut(&dest_channel_id)
            .ok_or(anyhow!("Channel not found: {}", dest_channel_id))?;

        *channel.sequence.borrow_mut() += 1;

        match msg {
            IbcMsg::Transfer { .. } => bail!("Transfer packet not supported yet"),
            IbcMsg::SendPacket { data, timeout, .. } => match &channel.local.port {
                IbcPort::Contract(contract) => {
                    let code_id = self.app.contract_data(&contract)?.code_id;
                    let ibc_details = self
                        .code_ids
                        .get(&code_id)
                        .ok_or(anyhow!("Code ID not found"))?;

                    let msg = IbcPacketReceiveMsg::new(
                        IbcPacket::new(
                            data.clone(),
                            channel.remote.as_endpoint()?,
                            channel.local.as_endpoint()?,
                            *channel.sequence.borrow(),
                            timeout.clone(),
                        ),
                        relayer.clone(),
                    );

                    let mut ack: Option<Binary> = None;

                    let response = self.app.use_contract(&contract, |deps, env| {
                        let res = ibc_details.ibc_packet_receive(deps, env, msg.clone())?;

                        if let Some(ack_data) = &res.acknowledgement {
                            ack = Some(ack_data.clone());
                        }

                        Ok(res).into_app_response()
                    })?;

                    Ok(PacketReceiveResponse { ack, response })
                }
                IbcPort::Module(_) => todo!(),
            },
            IbcMsg::CloseChannel { .. } => todo!(),
            _ => todo!(),
        }
    }

    pub fn packet_ack(
        &mut self,
        ack: Binary,
        original_msg: &IbcMsg,
        relayer: &Addr,
    ) -> AppResult<AppResponse> {
        let channel = original_msg.get_src_channel();
        let channel = self
            .channels
            .get(&channel.as_channel_number()?)
            .ok_or(anyhow!("Channel not found: {}", channel))?;

        match original_msg {
            IbcMsg::Transfer { .. } => todo!(),
            IbcMsg::SendPacket { data, timeout, .. } => match &channel.local.port {
                IbcPort::Contract(contract) => {
                    let code_id = self.app.contract_data(&contract)?.code_id;
                    let ibc_details = self
                        .code_ids
                        .get(&code_id)
                        .ok_or(anyhow!("Code ID not found"))?;

                    let msg = IbcPacketAckMsg::new(
                        IbcAcknowledgement::new(ack),
                        IbcPacket::new(
                            data.clone(),
                            channel.local.as_endpoint()?,
                            channel.remote.as_endpoint()?,
                            *channel.sequence.borrow(),
                            timeout.clone(),
                        ),
                        relayer.clone(),
                    );

                    self.app.use_contract(&contract, |deps, env| {
                        ibc_details
                            .ibc_packet_ack(deps, env, msg.clone())
                            .into_app_response()
                    })
                }
                IbcPort::Module(_) => todo!(),
            },
            IbcMsg::CloseChannel { .. } => todo!(),
            _ => todo!(),
        }
    }

    pub fn get_dest_channel_from_msg(&self, msg: &IbcMsg) -> AppResult<u64> {
        let channel_id = msg.get_src_channel().as_channel_number()?;
        let channel = self
            .channels
            .get(&channel_id)
            .ok_or(anyhow!("Channel not found: {}", channel_id))?;
        channel.remote.channel_id()
    }
}

pub struct PacketReceiveResponse {
    pub ack: Option<Binary>,
    pub response: AppResponse,
}
