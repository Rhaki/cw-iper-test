use std::{cell::RefCell, collections::BTreeMap, rc::Rc, u64};

use crate::{
    ibc::IbcChannelWrapper,
    ibc_app::InfallibleResult,
    ibc_application::{IbcApplication, PacketReceiveFailing, PacketReceiveOk},
    router::{RouterWrapper, UseRouter, UseRouterResponse},
};

use anyhow::{anyhow, bail};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    from_json, Addr, Api, Binary, BlockInfo, CustomMsg, CustomQuery, Empty, IbcAcknowledgement,
    IbcChannelConnectMsg, IbcChannelOpenMsg, IbcEndpoint, IbcMsg, IbcPacketAckMsg,
    IbcPacketReceiveMsg, IbcQuery, IbcTimeout, Querier, Storage,
};
use cw_multi_test::{AppResponse, CosmosRouter, Ibc, Module};
use cw_storage_plus::Item;
use serde::de::DeserializeOwned;

use crate::{
    error::AppResult,
    ibc::{IbcMsgExt, IbcPort},
    ibc_app::SharedChannels,
    router_closure,
};

pub const PENDING_PACKETS: Item<BTreeMap<u64, IbcPacketType>> = Item::new("pending_packets");

#[derive(Default)]
pub struct IbcModule {
    pub applications: BTreeMap<String, Rc<RefCell<dyn IbcApplication>>>,
    pub channels: SharedChannels,
}

impl IbcModule {
    fn load_application(
        &self,
        name: impl Into<String> + Clone,
    ) -> AppResult<&Rc<RefCell<dyn IbcApplication>>> {
        self.applications
            .get(&name.clone().into())
            .ok_or(anyhow!("application not found: {}", name.into()))
    }

    pub fn open_channel<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        application: &str,
        msg: IbcChannelOpenMsg,
    ) -> AppResult<AppResponse>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        let rc_storage = Rc::new(RefCell::new(storage));

        self.load_application(application)?.borrow().open_channel(
            api,
            block,
            &RouterWrapper::new(&router_closure!(router, api, rc_storage, block)),
            rc_storage.clone(),
            msg,
        )
    }

    pub fn channel_connect<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        application: &str,
        msg: IbcChannelConnectMsg,
    ) -> AppResult<AppResponse>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        let rc_storage = Rc::new(RefCell::new(storage));

        self.load_application(application)?
            .borrow()
            .channel_connect(
                api,
                block,
                &RouterWrapper::new(&router_closure!(router, api, rc_storage, block)),
                rc_storage.clone(),
                msg,
            )
    }

    pub fn packet_receive<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        application: &str,
        packet: IbcPacketReceiveMsg,
    ) -> InfallibleResult<PacketReceiveOk, PacketReceiveFailing>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        let rc_storage = Rc::new(RefCell::new(storage));

        self.load_application(application)
            .unwrap()
            .borrow()
            .packet_receive(
                api,
                block,
                &RouterWrapper::new(&router_closure!(router, api, rc_storage, block)),
                rc_storage.clone(),
                packet.clone(),
            )
    }

    pub fn packet_ack<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        application: &str,
        msg: AckPacket,
    ) -> AppResult<AppResponse>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        let rc_storage = Rc::new(RefCell::new(storage));

        self.load_application(application)?.borrow().packet_ack(
            api,
            block,
            &RouterWrapper::new(&router_closure!(router, api, rc_storage, block)),
            rc_storage.clone(),
            msg,
        )
    }

    pub fn packet_timeout<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        application: &str,
        msg: TimeoutPacket,
    ) -> AppResult<AppResponse>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        let rc_storage = Rc::new(RefCell::new(storage));

        self.load_application(application)?.borrow().packet_timeout(
            api,
            block,
            &RouterWrapper::new(&router_closure!(router, api, rc_storage, block)),
            rc_storage.clone(),
            msg,
        )
    }
}

impl Module for IbcModule {
    type ExecT = IbcMsg;
    type QueryT = IbcQuery;
    type SudoT = Empty;

    fn execute<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        sender: Addr,
        msg: Self::ExecT,
    ) -> AppResult<AppResponse>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        let channel = self.channels.borrow().get(msg.get_src_channel())?.clone();
        let rc_storage = Rc::new(RefCell::new(storage));

        if let IbcPort::Module(name) = &channel.local.port {
            self.load_application(name)?
                .borrow()
                .handle_outgoing_packet(
                    api,
                    block,
                    sender,
                    &RouterWrapper::new(&router_closure!(router, api, rc_storage, block)),
                    rc_storage.clone(),
                    msg.clone(),
                    channel,
                )
        } else {
            emit_packet_boxed(msg.into_packet(&sender, &channel)?, &rc_storage)?;
            Ok(AppResponse::default())
        }
    }

    fn query(
        &self,
        _api: &dyn Api,
        _storage: &dyn Storage,
        _querier: &dyn Querier,
        _block: &BlockInfo,
        _request: Self::QueryT,
    ) -> AppResult<Binary> {
        todo!()
    }

    fn sudo<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        _msg: Self::SudoT,
    ) -> AppResult<AppResponse>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        todo!()
    }
}

impl Ibc for IbcModule {}

#[cw_serde]
pub enum IbcPacketType {
    AckPacket(AckPacket),
    OutgoingPacket(OutgoingPacket),
    OutgoinPacketRaw(OutgoingPacketRaw),
    CloseChannel { channel_id: String },
    Timeout(TimeoutPacket),
}

impl IbcPacketType {
    pub fn get_channel_to_deliver(&self) -> AppResult<String> {
        match self {
            IbcPacketType::AckPacket(packet) => Ok(packet.get_src_channel()),
            IbcPacketType::OutgoingPacket(packet) => Ok(packet.get_dest_channel()),
            IbcPacketType::CloseChannel { .. } => {
                bail!("Unexpected error: Channel to deliver can't set for CloseChannel")
            }
            IbcPacketType::OutgoinPacketRaw(..) => {
                bail!("Unexpected error: Channel to deliver can't set for CloseChannel")
            }
            IbcPacketType::Timeout(packet) => {
                Ok(packet.original_packet.packet.src.channel_id.clone())
            }
        }
    }

    pub fn get_local_channel_id(&self) -> String {
        match self {
            IbcPacketType::AckPacket(packet) => {
                packet.original_packet.packet.dest.channel_id.clone()
            }
            IbcPacketType::OutgoingPacket(packet) => packet.src.channel_id.clone(),
            IbcPacketType::CloseChannel { channel_id } => channel_id.clone(),
            IbcPacketType::OutgoinPacketRaw(packet) => packet.src_channel.clone(),
            IbcPacketType::Timeout(packet) => packet.original_packet.packet.dest.channel_id.clone(),
        }
    }
}

#[cw_serde]
pub struct OutgoingPacket {
    pub data: Binary,
    pub src: IbcEndpoint,
    pub dest: IbcEndpoint,
    pub timeout: IbcTimeout,
}

#[cw_serde]
pub struct OutgoingPacketRaw {
    pub data: Binary,
    pub src_port: String,
    pub src_channel: String,
    pub timeout: IbcTimeout,
}

impl OutgoingPacketRaw {
    pub fn into_full_packet(self, channel: &IbcChannelWrapper) -> AppResult<OutgoingPacket> {
        Ok(OutgoingPacket {
            data: self.data,
            src: channel.local.as_endpoint()?,
            dest: channel.remote.as_endpoint()?,
            timeout: self.timeout,
        })
    }
}

#[cw_serde]
pub struct AckPacket {
    pub ack: Binary,
    pub original_packet: IbcPacketReceiveMsg,
    pub success: bool,
    pub relayer: Option<Addr>,
}

#[cw_serde]
pub struct TimeoutPacket {
    pub original_packet: IbcPacketReceiveMsg,
    pub relayer: Option<Addr>,
}

impl AckPacket {
    pub fn get_src_channel(&self) -> String {
        self.original_packet.packet.src.channel_id.clone()
    }

    pub fn into_msg(self, relayer: Addr) -> IbcPacketAckMsg {
        IbcPacketAckMsg::new(
            IbcAcknowledgement::new(self.ack),
            self.original_packet.packet,
            relayer,
        )
    }
}

#[cw_serde]
pub struct AckResponse {
    pub ack: Option<Binary>,
    pub success: bool,
}

impl OutgoingPacket {
    pub fn get_dest_channel(&self) -> String {
        self.dest.channel_id.clone()
    }

    pub fn get_src_channel(&self) -> String {
        self.src.channel_id.clone()
    }
}

pub fn emit_packet_boxed(
    packet: IbcPacketType,
    rc_storage: &Rc<RefCell<&mut dyn Storage>>,
) -> AppResult<()> {
    let mut packets = PENDING_PACKETS
        .load(*rc_storage.borrow())
        .unwrap_or_default();
    let new_key = packets.last_key_value().map(|(k, _)| *k).unwrap_or(0) + 1;
    packets.insert(new_key, packet);
    PENDING_PACKETS.save(*rc_storage.borrow_mut(), &packets)?;
    Ok(())
}

pub fn emit_packet(packet: IbcPacketType, storage: &mut dyn Storage) -> AppResult<()> {
    let mut packets = PENDING_PACKETS.load(storage).unwrap_or_default();
    let new_key = packets.last_key_value().map(|(k, _)| *k).unwrap_or(0) + 1;
    packets.insert(new_key, packet);
    PENDING_PACKETS.save(storage, &packets)?;
    Ok(())
}
