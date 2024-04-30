use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use anyhow::{anyhow, bail};
use cosmwasm_std::{
    testing::MockStorage, Addr, Api, Binary, CustomMsg, CustomQuery, Empty, IbcChannel,
    IbcChannelConnectMsg, IbcChannelOpenMsg, IbcPacket, IbcPacketReceiveMsg, IbcPacketTimeoutMsg,
    Storage,
};
use cw_multi_test::{
    transactional, App, AppResponse, Bank, BankKeeper, Distribution, DistributionKeeper,
    FailingModule, Gov, GovFailingModule, MockApiBech32, Module, StakeKeeper, Staking, Stargate,
    StorageTransaction, Wasm, WasmKeeper,
};
use serde::de::DeserializeOwned;

use crate::{
    contracts::{IbcContract, MultiContract},
    error::AppResult,
    ibc::{
        Channels, IbcChannelCreator, IbcChannelExt, IbcChannelStatus, IbcChannelWrapper, IbcPort,
    },
    ibc_module::{
        emit_packet, AckPacket, AckResponse, IbcModule, IbcPacketType, OutgoingPacket,
        TimeoutPacket, PENDING_PACKETS,
    },
    response::IntoResponse,
    stargate::StargateModule,
};

pub type SharedChannels = Rc<RefCell<Channels>>;
pub type BaseIbcApp = IbcApp<
    BankKeeper,
    MockApiBech32,
    MockStorage,
    FailingModule<Empty, Empty, Empty>,
    WasmKeeper<Empty, Empty>,
    StakeKeeper,
    DistributionKeeper,
    IbcModule,
    GovFailingModule,
    StargateModule,
>;

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
    CustomT::ExecT: CustomMsg + DeserializeOwned + 'static,
    CustomT::QueryT: CustomQuery + DeserializeOwned + 'static,
    StargateT: Stargate + 'static,
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
    pub fn get_pending_packet(&self, packet_id: u64) -> AppResult<IbcPacketType> {
        let packets = PENDING_PACKETS.load(self.app.storage())?;
        packets
            .get(&packet_id)
            .cloned()
            .ok_or(anyhow!("Packet not found"))
    }

    pub fn get_pending_packets(&self) -> AppResult<BTreeMap<u64, IbcPacketType>> {
        let packets = PENDING_PACKETS.load(self.app.storage()).unwrap_or_default();
        Ok(packets)
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

    pub fn incoming_packet(&mut self, packet: IbcPacketType) -> AppResult<MayResponse> {
        match packet {
            IbcPacketType::AckPacket(packet) => Ok(MayResponse::Ok(self.packet_ack(packet)?)),
            IbcPacketType::OutgoingPacket(packet) => self.packet_receive(packet),
            IbcPacketType::OutgoinPacketRaw(packet) => {
                let channel = self
                    .channels
                    .borrow()
                    .get(packet.src_channel.clone())?
                    .clone();

                self.packet_receive(packet.into_full_packet(&channel)?)
            }
            IbcPacketType::Timeout(packet) => Ok(MayResponse::Ok(self.packet_timeout(packet)?)),
            IbcPacketType::CloseChannel { .. } => unimplemented!("Close channel is unimplemented"),
        }
    }

    pub fn packet_receive(&mut self, packet: OutgoingPacket) -> AppResult<MayResponse> {
        let mut channels = self.channels.borrow_mut();

        let channel = channels.get_mut(packet.dest.channel_id.clone())?;

        *channel.sequence.borrow_mut() += 1;

        let msg = IbcPacketReceiveMsg::new(
            IbcPacket::new(
                packet.data.clone(),
                channel.remote.as_endpoint()?,
                channel.local.as_endpoint()?,
                *channel.sequence.borrow(),
                packet.timeout.clone(),
            ),
            self.relayer.clone(),
        );

        if let Err(err) = self.check_timeout(&packet) {
            emit_packet(
                IbcPacketType::Timeout(TimeoutPacket {
                    original_packet: msg,
                    relayer: None,
                }),
                self.app.storage_mut(),
            )?;
            return Ok(MayResponse::Err(err.to_string()));
        }

        match &channel.local.port {
            IbcPort::Contract(contract) => {
                let code_id = self.app.contract_data(contract)?.code_id;
                let ibc_details = self
                    .code_ids
                    .get(&code_id)
                    .ok_or(anyhow!("Code ID not found"))?;

                let mut ack: Option<Binary> = None;

                let response = self.app.use_contract(contract, |mut deps, env| {
                    let res = ibc_details.ibc_packet_receive(deps.branch(), env, msg.clone())?;

                    ack = res.acknowledgement.clone();

                    Ok(res).into_app_response()
                })?;

                if let Some(ack) = ack {
                    emit_packet(
                        IbcPacketType::AckPacket(AckPacket {
                            ack,
                            original_packet: msg,
                            // Mock as true for now. This field should not used on contract trigger on src chain
                            success: true,
                            relayer: None,
                        }),
                        self.app.storage_mut(),
                    )?;
                }

                Ok(MayResponse::Ok(response))
            }
            IbcPort::Module(name) => {
                let (api, store, block, router) = self.app.use_parts();

                let (ack_response, result) =
                    match infallible_transactional(&mut *store, |write_cache, _| {
                        router.ibc.packet_receive(
                            &*api,
                            write_cache,
                            router,
                            &*block,
                            name,
                            msg.clone(),
                        )
                    }) {
                        InfallibleResult::Ok(data) => (
                            AckResponse {
                                ack: data.ack,
                                success: true,
                            },
                            MayResponse::Ok(data.response),
                        ),
                        InfallibleResult::Err(data) => (
                            AckResponse {
                                ack: data.ack,
                                success: false,
                            },
                            MayResponse::Err(data.error),
                        ),
                    };

                if let Some(ack) = ack_response.ack {
                    emit_packet(
                        IbcPacketType::AckPacket(AckPacket {
                            ack,
                            original_packet: msg,
                            success: ack_response.success,
                            relayer: None,
                        }),
                        self.app.storage_mut(),
                    )?;
                }

                Ok(result)
            }
        }
    }

    pub fn packet_ack(&mut self, mut packet: AckPacket) -> AppResult<AppResponse> {
        let channel = packet.get_src_channel();

        let channels = self.channels.borrow();

        let channel = channels.get(channel)?;

        match &channel.local.port {
            IbcPort::Contract(contract) => {
                let code_id = self.app.contract_data(contract)?.code_id;
                let ibc_details = self
                    .code_ids
                    .get(&code_id)
                    .ok_or(anyhow!("Code ID not found"))?;

                self.app.use_contract(contract, |deps, env| {
                    ibc_details
                        .ibc_packet_ack(deps, env, packet.into_msg(self.relayer.clone()))
                        .into_app_response()
                })
            }
            IbcPort::Module(name) => {
                let (api, store, block, router) = self.app.use_parts();

                packet.relayer = Some(self.relayer.clone());

                transactional(&mut *store, |write_cache, _| {
                    router
                        .ibc
                        .packet_ack(&*api, write_cache, router, &*block, name, packet.clone())
                })
            }
        }
    }

    pub fn packet_timeout(&mut self, packet: TimeoutPacket) -> AppResult<AppResponse> {
        let channel = packet.original_packet.packet.src.channel_id.clone();

        let channels = self.channels.borrow();

        let channel = channels.get(channel)?;

        match &channel.local.port {
            IbcPort::Contract(contract) => {
                let code_id = self.app.contract_data(contract)?.code_id;
                let ibc_details = self
                    .code_ids
                    .get(&code_id)
                    .ok_or(anyhow!("Code ID not found"))?;

                let msg = IbcPacketTimeoutMsg::new(
                    IbcPacket::new(
                        packet.original_packet.packet.data.clone(),
                        channel.local.as_endpoint()?,
                        channel.remote.as_endpoint()?,
                        *channel.sequence.borrow(),
                        packet.original_packet.packet.timeout.clone(),
                    ),
                    self.relayer.clone(),
                );

                self.app.use_contract(contract, |deps, env| {
                    ibc_details
                        .ibc_packet_timeout(deps, env, msg.clone())
                        .into_app_response()
                })
            }
            IbcPort::Module(name) => {
                let (api, store, block, router) = self.app.use_parts();

                transactional(&mut *store, |write_cache, _| {
                    router
                        .ibc
                        .packet_timeout(&*api, write_cache, router, &*block, name, packet)
                })
            }
        }
    }

    pub fn get_next_channel_id(&self) -> u64 {
        self.channels.borrow().next_key()
    }

    fn check_timeout(&self, packet: &OutgoingPacket) -> AppResult<()> {
        let height = packet
            .timeout
            .block()
            .map(|val| val.height)
            .unwrap_or_default();

        let nanos = packet
            .timeout
            .timestamp()
            .map(|val| val.nanos())
            .unwrap_or_default();

        let invalid = match (height, nanos) {
            (0, 0) => true,
            (0, nanos) => self.app.block_info().time.nanos() > nanos,
            (height, 0) => self.app.block_info().height > height,
            (height, nanos) => {
                !(self.app.block_info().time.nanos() > nanos
                    && self.app.block_info().height > height)
            }
        };

        if invalid {
            bail!("Packet has timed out");
        } else {
            Ok(())
        }
    }
}

pub trait IbcAppRef {
    fn chain_id(&self) -> &str;
    fn channel_connect(&mut self, channel_id: u64) -> AppResult<()>;
    fn get_next_channel_id(&self) -> u64;
    fn get_next_pending_packet(&self) -> AppResult<u64>;
    fn get_pending_packet(&self, packet_id: u64) -> AppResult<IbcPacketType>;
    fn get_pending_packets(&self) -> AppResult<BTreeMap<u64, IbcPacketType>>;
    fn open_channel(
        &mut self,
        local: &IbcChannelCreator,
        remote: &IbcChannelCreator,
        sequence: Rc<RefCell<u64>>,
    ) -> AppResult<IbcChannelWrapper>;
    fn incoming_packet(&mut self, packet: IbcPacketType) -> AppResult<MayResponse>;
    fn remove_packet(&mut self, packet_id: u64) -> AppResult<()>;
    fn some_pending_packets(&self) -> bool;
    fn get_channel_info(&self, local_channel_id: String) -> AppResult<IbcChannelWrapper>;
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
    CustomT::ExecT: CustomMsg + DeserializeOwned + 'static,
    CustomT::QueryT: CustomQuery + DeserializeOwned + 'static,
    StargateT: Stargate + 'static,
{
    fn chain_id(&self) -> &str {
        &self.chain_id
    }
    fn channel_connect(&mut self, channel_id: u64) -> AppResult<()> {
        self.channel_connect(channel_id)
    }

    fn get_next_channel_id(&self) -> u64 {
        self.get_next_channel_id()
    }
    fn get_next_pending_packet(&self) -> AppResult<u64> {
        self.get_next_pending_packet()
    }
    fn get_pending_packet(&self, packet_id: u64) -> AppResult<IbcPacketType> {
        self.get_pending_packet(packet_id)
    }

    fn get_pending_packets(&self) -> AppResult<BTreeMap<u64, IbcPacketType>> {
        self.get_pending_packets()
    }
    fn open_channel(
        &mut self,
        local: &IbcChannelCreator,
        remote: &IbcChannelCreator,
        sequence: Rc<RefCell<u64>>,
    ) -> AppResult<IbcChannelWrapper> {
        self.open_channel(local, remote, sequence)
    }

    fn incoming_packet(&mut self, packet: IbcPacketType) -> AppResult<MayResponse> {
        self.incoming_packet(packet)
    }

    fn remove_packet(&mut self, packet_id: u64) -> AppResult<()> {
        self.remove_packet(packet_id)
    }

    fn some_pending_packets(&self) -> bool {
        self.some_pending_packets()
    }

    fn get_channel_info(&self, local_channel_id: String) -> AppResult<IbcChannelWrapper> {
        self.channels.borrow().get(local_channel_id).cloned()
    }
}

pub fn infallible_transactional<F, T, E>(
    base: &mut dyn Storage,
    action: F,
) -> InfallibleResult<T, E>
where
    F: FnOnce(&mut dyn Storage, &dyn Storage) -> InfallibleResult<T, E>,
{
    let mut cache = StorageTransaction::new(base);
    let res = action(&mut cache, base);

    if let InfallibleResult::Ok(_) = res {
        cache.prepare().commit(base);
    }

    res
}

#[derive(Debug, Clone)]
pub enum InfallibleResult<T, E> {
    Ok(T),
    Err(E),
}

impl<T, E> InfallibleResult<T, E> {
    pub fn is_err(&self) -> bool {
        matches!(self, InfallibleResult::Err(_))
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, InfallibleResult::Ok(_))
    }
}

#[derive(Debug, Clone)]
pub enum MayResponse {
    Ok(AppResponse),
    Err(String),
}
