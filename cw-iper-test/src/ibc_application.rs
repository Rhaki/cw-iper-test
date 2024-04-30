use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{
    Addr, Api, Binary, BlockInfo, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcMsg,
    IbcPacketReceiveMsg, Storage,
};
use cw_multi_test::{AppResponse, MockApiBech32};

use crate::{
    error::AppResult,
    ibc::IbcChannelWrapper,
    ibc_app::InfallibleResult,
    ibc_module::{AckPacket, TimeoutPacket},
    router::RouterWrapper,
};
pub trait IbcApplication: IbcPortInterface {
    fn handle_outgoing_packet(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        sender: Addr,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcMsg,
        channel: IbcChannelWrapper,
    ) -> AppResult<AppResponse>;

    fn packet_receive(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcPacketReceiveMsg,
    ) -> InfallibleResult<PacketReceiveOk, PacketReceiveFailing>;

    fn packet_ack(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: AckPacket,
    ) -> AppResult<AppResponse>;

    fn packet_timeout(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: TimeoutPacket,
    ) -> AppResult<AppResponse>;

    fn open_channel(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcChannelOpenMsg,
    ) -> AppResult<AppResponse>;

    fn channel_connect(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcChannelConnectMsg,
    ) -> AppResult<AppResponse>;

    fn init(&self, api: &MockApiBech32, storage: &mut dyn Storage);
}

pub trait IbcPortInterface {
    fn port_name(&self) -> String;
}

#[derive(Debug, Clone)]
pub struct PacketReceiveOk {
    pub response: AppResponse,
    pub ack: Option<Binary>,
}

#[derive(Debug, Clone)]
pub struct PacketReceiveFailing {
    pub error: String,
    pub ack: Option<Binary>,
}
