use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{
    Addr, Api, BlockInfo, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcMsg, IbcPacketAckMsg, Storage,
};
use cw_multi_test::AppResponse;

use crate::{
    error::AppResult,
    ibc_app::PacketReceiveResponse,
    ibc_module::{UseRouter, UseRouterResponse},
};

mod ics20;

pub use ics20::Ics20;

pub trait IbcApplication {
    fn port_name(&self) -> String;

    fn handle_outgoing_packet(
        &self,
        msg: IbcMsg,
        api: &dyn Api,
        block: &BlockInfo,
        sender: Addr,
        clos: &dyn Fn(UseRouter) -> AppResult<UseRouterResponse>,
        storage: Rc<RefCell<&mut dyn Storage>>,
    ) -> AppResult<AppResponse>;

    fn packet_receive(
        &self,
        msg: IbcMsg,
        api: &dyn Api,
        block: &BlockInfo,
        clos: &dyn Fn(UseRouter) -> AppResult<UseRouterResponse>,
        storage: Rc<RefCell<&mut dyn Storage>>,
    ) -> AppResult<PacketReceiveResponse>;

    fn packet_ack(
        &self,
        msg: IbcPacketAckMsg,
        api: &dyn Api,
        block: &BlockInfo,
        clos: &dyn Fn(UseRouter) -> AppResult<UseRouterResponse>,
        storage: Rc<RefCell<&mut dyn Storage>>,
    ) -> AppResult<AppResponse>;

    fn open_channel(
        &self,
        msg: IbcChannelOpenMsg,
        api: &dyn Api,
        block: &BlockInfo,
        clos: &dyn Fn(UseRouter) -> AppResult<UseRouterResponse>,
        storage: Rc<RefCell<&mut dyn Storage>>,
    ) -> AppResult<AppResponse>;

    fn channel_connect(
        &self,
        msg: IbcChannelConnectMsg,
        api: &dyn Api,
        block: &BlockInfo,
        clos: &dyn Fn(UseRouter) -> AppResult<UseRouterResponse>,
        storage: Rc<RefCell<&mut dyn Storage>>,
    ) -> AppResult<AppResponse>;
}
