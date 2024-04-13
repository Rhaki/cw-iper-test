use std::{cell::RefCell, rc::Rc};

use super::IbcApplication;
use cosmwasm_std::{
    Addr, Api, BlockInfo, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcMsg, IbcPacketAckMsg, Storage,
};
use cw_multi_test::AppResponse;

use crate::{
    error::AppResult,
    ibc_app::PacketReceiveResponse,
    ibc_module::{UseRouter, UseRouterResponse},
};

#[derive(Default)]
pub struct Ics20;

impl Ics20 {
    pub const NAME: &'static str = "transfer";
}

impl IbcApplication for Ics20 {
    fn port_name(&self) -> String {
        Ics20::NAME.to_string()
    }

    fn handle_outgoing_packet(
        &self,
        msg: IbcMsg,
        api: &dyn Api,
        block: &BlockInfo,
        sender: Addr,
        clos: &dyn Fn(UseRouter) -> AppResult<UseRouterResponse>,
        storage: Rc<RefCell<&mut dyn Storage>>,
    ) -> AppResult<AppResponse> {
        todo!()
    }

    fn packet_receive(
        &self,
        msg: IbcMsg,
        api: &dyn Api,
        block: &BlockInfo,
        clos: &dyn Fn(UseRouter) -> AppResult<UseRouterResponse>,
        storage: Rc<RefCell<&mut dyn Storage>>,
    ) -> AppResult<PacketReceiveResponse> {
        todo!()
    }

    fn packet_ack(
        &self,
        msg: IbcPacketAckMsg,
        api: &dyn Api,
        block: &BlockInfo,
        clos: &dyn Fn(UseRouter) -> AppResult<UseRouterResponse>,
        storage: Rc<RefCell<&mut dyn Storage>>,
    ) -> AppResult<AppResponse> {
        todo!()
    }

    fn open_channel(
        &self,
        msg: IbcChannelOpenMsg,
        api: &dyn Api,
        block: &BlockInfo,
        clos: &dyn Fn(UseRouter) -> AppResult<UseRouterResponse>,
        storage: Rc<RefCell<&mut dyn Storage>>,
    ) -> AppResult<AppResponse> {
        println!("channel_opened");
        Ok(AppResponse::default())
    }

    fn channel_connect(
        &self,
        msg: IbcChannelConnectMsg,
        api: &dyn Api,
        block: &BlockInfo,
        clos: &dyn Fn(UseRouter) -> AppResult<UseRouterResponse>,
        storage: Rc<RefCell<&mut dyn Storage>>,
    ) -> AppResult<AppResponse> {
        println!("channel_connect");
        Ok(AppResponse::default())
    }
}
