use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use anyhow::{anyhow, bail};
use cosmwasm_std::{
    Addr, Api, Binary, CustomMsg, CustomQuery, IbcAcknowledgement, IbcChannel,
    IbcChannelConnectMsg, IbcChannelOpenMsg, IbcMsg, IbcPacket, IbcPacketAckMsg,
    IbcPacketReceiveMsg, Storage,
};
use cw_multi_test::{
    transactional, App, AppResponse, Bank, Distribution, Gov, Module, Staking, Stargate, Wasm,
};
use serde::de::DeserializeOwned;

use crate::{
    contracts::{IbcContract, MultiContract},
    error::AppResult,
    ibc::{
        Channels, IbcChannelCreator, IbcChannelExt, IbcChannelStatus, IbcChannelWrapper, IbcMsgExt,
        IbcPort,
    },
    ibc_module::{IbcModule, PENDING_PACKETS},
    response::IntoResponse,
};

pub type SharedChannels = Rc<RefCell<Channels>>;

pub struct IbcApp<BankT, ApiT, StorageT, CustomT, WasmT, StakingT, DistrT, IbcT, GovT, StargateT>
where
    CustomT: Module,
{
    pub relayer: Addr,
    pub chain_id: String,
    pub app: App<BankT, ApiT, StorageT, CustomT, WasmT, StakingT, DistrT, IbcT, GovT, StargateT>,
    pub code_ids: BTreeMap<u64, Box<dyn IbcContract<CustomT::ExecT, CustomT::QueryT>>>,
    pub channels: SharedChannels,
}

impl<BankT, ApiT, StorageT, CustomT, WasmT, StakingT, DistrT, GovT, StargateT>
    IbcApp<BankT, ApiT, StorageT, CustomT, WasmT, StakingT, DistrT, IbcModule, GovT, StargateT>
where
    BankT: Bank + 'static,
    ApiT: Api + 'static,
    StorageT: Storage + 'static,
    CustomT: Module + 'static,
    WasmT: Wasm<CustomT::ExecT, CustomT::QueryT> + 'static,
    StakingT: Staking + 'static,
    DistrT: Distribution + 'static,
    GovT: Gov + 'static,
    StargateT: Stargate + 'static,
    CustomT::ExecT: CustomMsg + DeserializeOwned + 'static,
    CustomT::QueryT: CustomQuery + DeserializeOwned + 'static,
{
    pub fn store_ibc_code(
        &mut self,
        contract: MultiContract<CustomT::ExecT, CustomT::QueryT>,
    ) -> u64 {
        let code_id = self.app.store_code(contract.base);
        if let Some(ibc) = contract.ibc {
            self.code_ids.insert(code_id, ibc);
        }
        code_id
    }
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
            .map(|(k, _)| *k)
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

    pub fn open_channel(
        &mut self,
        local: &IbcChannelCreator,
        remote: &IbcChannelCreator,
        sequence: Rc<RefCell<u64>>,
    ) -> AppResult<IbcChannelWrapper> {
        let channel_wrapper = IbcChannelWrapper::new(local.clone(), remote.clone(), sequence);

        let msg = IbcChannelOpenMsg::new_init(IbcChannel::new_from_creators(local, remote)?);
        match &local.port {
            IbcPort::Contract(contract) => {
                let code_id = self.app.contract_data(contract)?.code_id;
                let ibc_details = self
                    .code_ids
                    .get(&code_id)
                    .ok_or(anyhow!("Code ID not found"))?;

                self.app.use_contract(contract, |deps, env| {
                    ibc_details
                        .ibc_channel_open(deps, env, msg)
                        .into_app_response()
                })?;
            }
            IbcPort::Module(name) => {
                let (api, store, block, router) = self.app.use_parts();

                transactional(&mut *store, |write_cache, _| {
                    router
                        .ibc
                        .open_channel(&*api, write_cache, router, &*block, name, msg.clone())
                })?;
            }
        }

        self.channels
            .borrow_mut()
            .insert(channel_wrapper.local.channel_id()?, channel_wrapper.clone())?;

        Ok(channel_wrapper)
    }

    pub fn channel_connect(&mut self, channel_id: u64) -> AppResult<()> {
        let mut channels = self.channels.borrow_mut();
        let channel = channels.get_mut(channel_id)?;
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
                let code_id = self.app.contract_data(contract)?.code_id;
                let ibc_details = self
                    .code_ids
                    .get(&code_id)
                    .ok_or(anyhow!("Code ID not found"))?;

                self.app.use_contract(contract, |deps, env| {
                    ibc_details
                        .ibc_channel_connect(deps, env, msg)
                        .into_app_response()
                })?;
            }
            IbcPort::Module(name) => {
                let (api, store, block, router) = self.app.use_parts();

                transactional(&mut *store, |write_cache, _| {
                    router.ibc.channel_connect(
                        &*api,
                        write_cache,
                        router,
                        &*block,
                        name,
                        msg.clone(),
                    )
                })?;
            }
        }

        channel.status.to_next_status()?;

        Ok(())
    }

    pub fn packet_receive(
        &mut self,
        msg: &IbcMsg,
        dest_channel_id: u64,
    ) -> AppResult<PacketReceiveResponse> {
        let mut channels = self.channels.borrow_mut();

        let channel = channels.get_mut(dest_channel_id)?;

        *channel.sequence.borrow_mut() += 1;

        match msg {
            IbcMsg::Transfer { .. } => bail!("Transfer packet not supported yet"),
            IbcMsg::SendPacket { data, timeout, .. } => match &channel.local.port {
                IbcPort::Contract(contract) => {
                    let code_id = self.app.contract_data(contract)?.code_id;
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
                        self.relayer.clone(),
                    );

                    let mut ack: Option<Binary> = None;

                    let response = self.app.use_contract(contract, |deps, env| {
                        let res = ibc_details.ibc_packet_receive(deps, env, msg.clone())?;

                        if let Some(ack_data) = &res.acknowledgement {
                            ack = Some(ack_data.clone());
                        }

                        Ok(res).into_app_response()
                    })?;

                    Ok(PacketReceiveResponse { ack, response })
                }
                IbcPort::Module(name) => {
                    let (api, store, block, router) = self.app.use_parts();

                    transactional(&mut *store, |write_cache, _| {
                        router.ibc.packet_receive(
                            &*api,
                            write_cache,
                            router,
                            &*block,
                            name,
                            msg.clone(),
                        )
                    })
                }
            },
            IbcMsg::CloseChannel { .. } => todo!(),
            _ => todo!(),
        }
    }

    pub fn packet_ack(&mut self, ack: Binary, original_msg: &IbcMsg) -> AppResult<AppResponse> {
        let channel = original_msg.get_src_channel();

        let channels = self.channels.borrow();

        let channel = channels.get(channel)?;

        match original_msg {
            IbcMsg::Transfer { .. } => todo!(),
            IbcMsg::SendPacket { data, timeout, .. } => {
                let msg = IbcPacketAckMsg::new(
                    IbcAcknowledgement::new(ack),
                    IbcPacket::new(
                        data.clone(),
                        channel.local.as_endpoint()?,
                        channel.remote.as_endpoint()?,
                        *channel.sequence.borrow(),
                        timeout.clone(),
                    ),
                    self.relayer.clone(),
                );

                match &channel.local.port {
                    IbcPort::Contract(contract) => {
                        let code_id = self.app.contract_data(contract)?.code_id;
                        let ibc_details = self
                            .code_ids
                            .get(&code_id)
                            .ok_or(anyhow!("Code ID not found"))?;

                        self.app.use_contract(contract, |deps, env| {
                            ibc_details
                                .ibc_packet_ack(deps, env, msg.clone())
                                .into_app_response()
                        })
                    }
                    IbcPort::Module(name) => {
                        let (api, store, block, router) = self.app.use_parts();

                        transactional(&mut *store, |write_cache, _| {
                            router.ibc.packet_ack(
                                &*api,
                                write_cache,
                                router,
                                &*block,
                                name,
                                msg.clone(),
                            )
                        })
                    }
                }
            }
            IbcMsg::CloseChannel { .. } => todo!(),
            _ => todo!(),
        }
    }

    pub fn get_dest_channel_from_msg(&self, msg: &IbcMsg) -> AppResult<IbcChannelCreator> {
        let channel_id = msg.get_src_channel();
        self.channels
            .borrow()
            .get(channel_id)
            .map(|val| val.remote.clone())
    }

    pub fn get_next_channel_id(&self) -> u64 {
        self.channels.borrow().next_key()
    }
}

pub trait IbcAppRef {
    fn chain_id(&self) -> &str;
    fn channel_connect(&mut self, channel_id: u64) -> AppResult<()>;
    fn get_dest_channel_from_msg(&self, msg: &IbcMsg) -> AppResult<IbcChannelCreator>;
    fn get_next_channel_id(&self) -> u64;
    fn get_next_pending_packet(&self) -> AppResult<u64>;
    fn get_pending_packet(&self, packet_id: u64) -> AppResult<IbcMsg>;
    fn open_channel(
        &mut self,
        local: &IbcChannelCreator,
        remote: &IbcChannelCreator,
        sequence: Rc<RefCell<u64>>,
    ) -> AppResult<IbcChannelWrapper>;
    fn packet_ack(&mut self, ack: Binary, original_msg: &IbcMsg) -> AppResult<AppResponse>;
    fn packet_receive(
        &mut self,
        msg: &IbcMsg,
        dest_channel_id: u64,
    ) -> AppResult<PacketReceiveResponse>;
    fn remove_packet(&mut self, packet_id: u64) -> AppResult<()>;
    fn some_pending_packets(&self) -> bool;
}

impl<BankT, ApiT, StorageT, CustomT, WasmT, StakingT, DistrT, GovT, StargateT> IbcAppRef
    for IbcApp<BankT, ApiT, StorageT, CustomT, WasmT, StakingT, DistrT, IbcModule, GovT, StargateT>
where
    BankT: Bank + 'static,
    ApiT: Api + 'static,
    StorageT: Storage + 'static,
    CustomT: Module + 'static,
    WasmT: Wasm<CustomT::ExecT, CustomT::QueryT> + 'static,
    StakingT: Staking + 'static,
    DistrT: Distribution + 'static,
    GovT: Gov + 'static,
    StargateT: Stargate + 'static,
    CustomT::ExecT: CustomMsg + DeserializeOwned + 'static,
    CustomT::QueryT: CustomQuery + DeserializeOwned + 'static,
{
    fn chain_id(&self) -> &str {
        &self.chain_id
    }
    fn channel_connect(&mut self, channel_id: u64) -> AppResult<()> {
        self.channel_connect(channel_id)
    }
    fn get_dest_channel_from_msg(&self, msg: &IbcMsg) -> AppResult<IbcChannelCreator> {
        self.get_dest_channel_from_msg(msg)
    }
    fn get_next_channel_id(&self) -> u64 {
        self.get_next_channel_id()
    }
    fn get_next_pending_packet(&self) -> AppResult<u64> {
        self.get_next_pending_packet()
    }
    fn get_pending_packet(&self, packet_id: u64) -> AppResult<IbcMsg> {
        self.get_pending_packet(packet_id)
    }

    fn open_channel(
        &mut self,
        local: &IbcChannelCreator,
        remote: &IbcChannelCreator,
        sequence: Rc<RefCell<u64>>,
    ) -> AppResult<IbcChannelWrapper> {
        self.open_channel(local, remote, sequence)
    }
    fn packet_ack(&mut self, ack: Binary, original_msg: &IbcMsg) -> AppResult<AppResponse> {
        self.packet_ack(ack, original_msg)
    }

    fn packet_receive(
        &mut self,
        msg: &IbcMsg,
        dest_channel_id: u64,
    ) -> AppResult<PacketReceiveResponse> {
        self.packet_receive(msg, dest_channel_id)
    }

    fn remove_packet(&mut self, packet_id: u64) -> AppResult<()> {
        self.remove_packet(packet_id)
    }

    fn some_pending_packets(&self) -> bool {
        self.some_pending_packets()
    }
}

pub struct PacketReceiveResponse {
    pub ack: Option<Binary>,
    pub response: AppResponse,
}
