use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{
    Addr, Api, BlockInfo, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcMsg, IbcPacketAckMsg,
    IbcPacketReceiveMsg, Storage,
};
use cw_multi_test::AppResponse;

use crate::{
    error::AppResult,
    ibc::IbcChannelWrapper,
    response::AppResponseExt,
    router::RouterWrapper,
    stargate::{StargateApplication, StargateUrls},
};

use super::{IbcApplication, IbcPortInterface};

pub trait IbcAndStargate: IbcApplication + StargateApplication {}

pub enum MiddlewareResponse<T> {
    Stop(T),
    Continue(T),
}

pub trait Middleware {
    fn get_inner(&self) -> &dyn IbcAndStargate;

    fn mid_handle_outgoing_packet(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        sender: Addr,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcMsg,
        channel: IbcChannelWrapper,
    ) -> AppResult<MiddlewareResponse<AppResponse>>;

    fn mid_packet_receive(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcPacketReceiveMsg,
    ) -> AppResult<MiddlewareResponse<AppResponse>>;

    fn mid_packet_ack(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcPacketAckMsg,
    ) -> AppResult<MiddlewareResponse<AppResponse>>;

    fn mid_open_channel(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcChannelOpenMsg,
    ) -> AppResult<MiddlewareResponse<AppResponse>>;

    fn mid_channel_connect(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcChannelConnectMsg,
    ) -> AppResult<MiddlewareResponse<AppResponse>>;
}

impl<T> IbcPortInterface for T
where
    T: Middleware,
{
    fn port_name(&self) -> String {
        self.get_inner().port_name()
    }
}

impl<T> IbcApplication for T
where
    T: Middleware,
{
    fn handle_outgoing_packet(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        sender: Addr,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcMsg,
        channel: IbcChannelWrapper,
    ) -> AppResult<AppResponse> {
        match self.mid_handle_outgoing_packet(
            api,
            block,
            sender.clone(),
            router,
            storage.clone(),
            msg.clone(),
            channel.clone(),
        )? {
            MiddlewareResponse::Stop(response) => Ok(response),
            MiddlewareResponse::Continue(response) => {
                let sub_response = self
                    .get_inner()
                    .handle_outgoing_packet(api, block, sender, router, storage, msg, channel)?;
                Ok(response.merge(sub_response))
            }
        }
    }

    fn packet_receive(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcPacketReceiveMsg,
    ) -> AppResult<AppResponse> {
        match self.mid_packet_receive(api, block, router, storage.clone(), msg.clone())? {
            MiddlewareResponse::Stop(response) => Ok(response),
            MiddlewareResponse::Continue(response) => {
                let sub_response = self
                    .get_inner()
                    .packet_receive(api, block, router, storage, msg)?;
                Ok(response.merge(sub_response))
            }
        }
    }

    fn packet_ack(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcPacketAckMsg,
    ) -> AppResult<AppResponse> {
        match self.mid_packet_ack(api, block, router, storage.clone(), msg.clone())? {
            MiddlewareResponse::Stop(response) => Ok(response),
            MiddlewareResponse::Continue(response) => {
                let sub_response = self
                    .get_inner()
                    .packet_ack(api, block, router, storage, msg)?;
                Ok(response.merge(sub_response))
            }
        }
    }

    fn open_channel(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcChannelOpenMsg,
    ) -> AppResult<AppResponse> {
        match self.mid_open_channel(api, block, router, storage.clone(), msg.clone())? {
            MiddlewareResponse::Stop(response) => Ok(response),
            MiddlewareResponse::Continue(response) => {
                let sub_response = self
                    .get_inner()
                    .open_channel(api, block, router, storage, msg)?;
                Ok(response.merge(sub_response))
            }
        }
    }

    fn channel_connect(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcChannelConnectMsg,
    ) -> AppResult<AppResponse> {
        match self.mid_channel_connect(api, block, router, storage.clone(), msg.clone())? {
            MiddlewareResponse::Stop(response) => Ok(response),
            MiddlewareResponse::Continue(response) => {
                let sub_response = self
                    .get_inner()
                    .channel_connect(api, block, router, storage, msg)?;
                Ok(response.merge(sub_response))
            }
        }
    }
}

impl<T> StargateApplication for T
where
    T: Middleware + StargateUrls,
{
    fn stargate_msg(
        &self,
        api: &dyn Api,
        storage: Rc<RefCell<&mut dyn Storage>>,
        router: &RouterWrapper,
        block: &BlockInfo,
        sender: Addr,
        type_url: String,
        data: cosmwasm_std::Binary,
    ) -> AppResult<AppResponse> {
        self.get_inner()
            .stargate_msg(api, storage, router, block, sender, type_url, data)
    }

    fn stargate_query(
        &self,
        api: &dyn Api,
        storage: &dyn Storage,
        querier: &dyn cosmwasm_std::Querier,
        block: &BlockInfo,
        request: cosmwasm_std::GrpcQuery,
    ) -> AppResult<cosmwasm_std::Binary> {
        self.get_inner()
            .stargate_query(api, storage, querier, block, request)
    }
}

impl<T> StargateUrls for T
where
    T: Middleware,
{
    fn stargate_name(&self) -> String {
        self.get_inner().stargate_name()
    }

    fn is_query_type_url(&self, type_url: String) -> bool {
        self.get_inner().is_query_type_url(type_url)
    }

    fn is_msg_type_url(&self, type_url: String) -> bool {
        self.get_inner().is_msg_type_url(type_url)
    }

    fn type_urls(&self) -> Vec<String> {
        self.get_inner().type_urls()
    }
}
