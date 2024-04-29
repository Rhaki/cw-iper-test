use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{
    Addr, Api, BlockInfo, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcMsg, IbcPacketAckMsg,
    IbcPacketReceiveMsg, IbcPacketTimeoutMsg, Storage,
};
use cw_multi_test::AppResponse;

use crate::{
    error::AppResult,
    ibc::IbcChannelWrapper,
    ibc_app::InfallibleResult,
    ibc_application::{IbcApplication, IbcPortInterface, PacketReceiveFailing, PacketReceiveOk},
    response::AppResponseExt,
    router::RouterWrapper,
    stargate::{StargateApplication, StargateName, StargateUrls},
};

pub trait IbcAndStargate: IbcApplication + StargateApplication {}

pub type MiddlewareUniqueResponse<T> = MiddlewareResponse<T, T>;

pub enum MiddlewareResponse<S, C> {
    Stop(S),
    Continue(C),
}

pub struct PacketToNext {
    pub packet: IbcPacketReceiveMsg,
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
    ) -> AppResult<MiddlewareUniqueResponse<AppResponse>>;

    fn mid_packet_receive_before(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        packet: IbcPacketReceiveMsg,
    ) -> InfallibleResult<MiddlewareResponse<PacketReceiveOk, PacketToNext>, PacketReceiveFailing>;

    fn mid_packet_receive_after(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        original_packet: IbcPacketReceiveMsg,
        forwarded_packet: IbcPacketReceiveMsg,
        forwarded_response: PacketReceiveOk,
    ) -> InfallibleResult<PacketReceiveOk, PacketReceiveFailing>;

    fn mid_packet_ack(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcPacketAckMsg,
    ) -> AppResult<MiddlewareUniqueResponse<AppResponse>>;

    fn mid_packet_timeout(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcPacketTimeoutMsg,
    ) -> AppResult<MiddlewareUniqueResponse<AppResponse>>;

    fn mid_open_channel(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcChannelOpenMsg,
    ) -> AppResult<MiddlewareUniqueResponse<AppResponse>>;

    fn mid_channel_connect(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcChannelConnectMsg,
    ) -> AppResult<MiddlewareUniqueResponse<AppResponse>>;
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
    T: Middleware + StargateUrls + 'static,
{
    fn init(&self, api: &cw_multi_test::MockApiBech32, storage: &mut dyn Storage) {
        self.get_inner().init(api, storage)
    }

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
        original_packet: IbcPacketReceiveMsg,
    ) -> InfallibleResult<PacketReceiveOk, PacketReceiveFailing> {
        match self.mid_packet_receive_before(
            api,
            block,
            router,
            storage.clone(),
            original_packet.clone(),
        ) {
            InfallibleResult::Ok(res) => match res {
                MiddlewareResponse::Stop(res) => InfallibleResult::Ok(res),
                MiddlewareResponse::Continue(next_packet) => {
                    let sub_response = self.get_inner().packet_receive(
                        api,
                        block,
                        router,
                        storage.clone(),
                        next_packet.packet.clone(),
                    );

                    match sub_response {
                        InfallibleResult::Ok(sub_response) => self.mid_packet_receive_after(
                            api,
                            block,
                            router,
                            storage,
                            original_packet,
                            next_packet.packet,
                            sub_response,
                        ),
                        InfallibleResult::Err(err) => InfallibleResult::Err(err),
                    }
                }
            },
            InfallibleResult::Err(err) => InfallibleResult::Err(err),
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

    fn packet_timeout(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcPacketTimeoutMsg,
    ) -> AppResult<AppResponse> {
        match self.mid_packet_timeout(api, block, router, storage.clone(), msg.clone())? {
            MiddlewareResponse::Stop(response) => Ok(response),
            MiddlewareResponse::Continue(response) => {
                let sub_response = self
                    .get_inner()
                    .packet_timeout(api, block, router, storage, msg)?;
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

impl<T> StargateName for T
where
    T: Middleware,
{
    fn stargate_name(&self) -> String {
        self.get_inner().stargate_name()
    }
}

impl<T> StargateApplication for T
where
    T: Middleware + StargateUrls + 'static,
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
        let res = self.get_inner().stargate_msg(
            api,
            storage.clone(),
            router,
            block,
            sender,
            type_url,
            data,
        )?;

        Ok(res)
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

impl<T> IbcAndStargate for T where T: IbcApplication + StargateApplication {}
