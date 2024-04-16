use super::{middleware::{IbcAndStargate, Middleware}, IbcApplication};

pub struct IbcHook {
    pub inner: Box<dyn IbcApplication>,
}

impl IbcHook {
    pub fn new<T: IbcApplication + 'static>(inner: T) -> Self {
        Self {
            inner: Box::new(inner),
        }
    }
}

impl Middleware for IbcHook{
    fn get_inner(&self) -> &dyn IbcAndStargate {
        todo!()
    }

    fn mid_handle_outgoing_packet(
        &self,
        api: &dyn cosmwasm_std::Api,
        block: &cosmwasm_std::BlockInfo,
        sender: cosmwasm_std::Addr,
        router: &crate::router::RouterWrapper,
        storage: std::rc::Rc<std::cell::RefCell<&mut dyn cosmwasm_std::Storage>>,
        msg: cosmwasm_std::IbcMsg,
        channel: crate::ibc::IbcChannelWrapper,
    ) -> crate::error::AppResult<super::middleware::MiddlewareResponse<cw_multi_test::AppResponse>> {
        todo!()
    }

    fn mid_packet_receive(
        &self,
        api: &dyn cosmwasm_std::Api,
        block: &cosmwasm_std::BlockInfo,
        router: &crate::router::RouterWrapper,
        storage: std::rc::Rc<std::cell::RefCell<&mut dyn cosmwasm_std::Storage>>,
        msg: cosmwasm_std::IbcPacketReceiveMsg,
    ) -> crate::error::AppResult<super::middleware::MiddlewareResponse<cw_multi_test::AppResponse>> {
        todo!()
    }

    fn mid_packet_ack(
        &self,
        api: &dyn cosmwasm_std::Api,
        block: &cosmwasm_std::BlockInfo,
        router: &crate::router::RouterWrapper,
        storage: std::rc::Rc<std::cell::RefCell<&mut dyn cosmwasm_std::Storage>>,
        msg: cosmwasm_std::IbcPacketAckMsg,
    ) -> crate::error::AppResult<super::middleware::MiddlewareResponse<cw_multi_test::AppResponse>> {
        todo!()
    }

    fn mid_open_channel(
        &self,
        api: &dyn cosmwasm_std::Api,
        block: &cosmwasm_std::BlockInfo,
        router: &crate::router::RouterWrapper,
        storage: std::rc::Rc<std::cell::RefCell<&mut dyn cosmwasm_std::Storage>>,
        msg: cosmwasm_std::IbcChannelOpenMsg,
    ) -> crate::error::AppResult<super::middleware::MiddlewareResponse<cw_multi_test::AppResponse>> {
        todo!()
    }

    fn mid_channel_connect(
        &self,
        api: &dyn cosmwasm_std::Api,
        block: &cosmwasm_std::BlockInfo,
        router: &crate::router::RouterWrapper,
        storage: std::rc::Rc<std::cell::RefCell<&mut dyn cosmwasm_std::Storage>>,
        msg: cosmwasm_std::IbcChannelConnectMsg,
    ) -> crate::error::AppResult<super::middleware::MiddlewareResponse<cw_multi_test::AppResponse>> {
        todo!()
    }
}