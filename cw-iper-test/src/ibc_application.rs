use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{
    Addr, Api, Binary, BlockInfo, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcMsg, IbcPacketAckMsg,
    IbcPacketReceiveMsg, Storage,
};
use cw_multi_test::AppResponse;

use crate::{error::AppResult, ibc::IbcChannelWrapper, router::RouterWrapper};
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
    ) -> AppResult<PacketReceiveResponse>;

    fn packet_ack(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcPacketAckMsg,
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
}

pub trait IbcPortInterface {
    fn port_name(&self) -> String;
}

pub struct PacketReceiveResponse {
    pub response: AppResponse,
    pub ack: Binary,
}
