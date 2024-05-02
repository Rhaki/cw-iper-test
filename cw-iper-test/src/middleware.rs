use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{
    Addr, Api, Binary, BlockInfo, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcMsg,
    IbcPacketReceiveMsg, Storage,
};
use cw_multi_test::AppResponse;

use crate::{
    error::AppResult,
    ibc::IbcChannelWrapper,
    ibc_application::{IbcApplication, IbcPortInterface, PacketReceiveFailing, PacketReceiveOk},
    ibc_module::{AckPacket, TimeoutPacket},
    iper_app::InfallibleResult,
    response::AppResponseExt,
    router::RouterWrapper,
    stargate::{StargateApplication, StargateName, StargateUrls},
};

pub trait IbcAndStargate: IbcApplication + StargateApplication {}

/// Enum rappresenting how the flow has to be controlled when triggering the `before` variant of [`Middleware`] functions.
/// - **Stop**: The inner application will not called and the Value `S` will be returned;
/// - **Continue**: The inner application will called. After the execution of the inner call, the `after` variant of the [`Middleware`] function will be called.

pub enum MiddlewareResponse<S, C> {
    /// The inner application will not called and the Value `S` will be returned.
    Stop(S),
    /// The inner application will called. After the execution of the inner call, the `after` variant of the [`Middleware`] function will be called.
    Continue(C),
}

/// This trait allow to reproduce the functionality of Middleware ibc application (like IbcHook).
/// [`Middleware`] allow to wrap another [`IbcApplication`] and and alter/implement functionality when one of the various functions that the [`IbcApplication`] implements is called.
///
/// The core logic about []
///
/// [`Middleware`] trait alredy implements [`IbcApplication`] and [`StargateApplication`] by default, so implementing [`Middleware`] implement also [`IbcApplication`] and [`StargateApplication`].
///
/// ## How it works
///
/// The core logic of the [`Middleware`] involves the implementation of `functions` that wrap the individual functions
/// of the [`IbcApplication`] into two distinct `functions`, called `before` and `after`. For example, considering
/// the [`IbcApplication::packet_receive`] function, [`Middleware`] managed through [`Middleware::mid_packet_receive_before`]
/// and [`Middleware::mid_packet_receive_after`] to handle actions before and after the execution of [`IbcApplication::packet_receive`].

/// Specifically, the `before` functions are called before the linked `function` of the `inner` [`IbcApplication`] is invoked.
/// These `before` functions return a type of [`MiddlewareResponse`], which offers two alternatives:

/// - **Stop**: The `inner function` will not be called, and the value is returned directly.
/// - **Continue**: The `inner function` will be called. The result of the `inner function` is then passed to the
///   corresponding `after` function of the Middleware, where further actions can be performed.

/// ## Realistic Example: Implementing [`IbcHook`](crate::ibc_applications::IbcHook)
///
/// In the [`IbcHook`](crate::ibc_applications::IbcHook), during [`Middleware::mid_packet_receive_before`], if the `memo` of the [`FungibleTokenPacketData`] is set
/// to trigger the `IBC hook`, the `packet` is modified, and the `sender` is set according to the [`IBC hook standard`](https://github.com/osmosis-labs/osmosis/tree/main/x/ibc-hooks#:~:text=Sender%3A%20We%20cannot,the%20local%20chain.).
/// This `packet` is then returned with `Continue`, and passed to the inner [`IbcApplication`] (e.g., `Ics20` or
/// another [`Middleware`] if there are multiple wraps).
///
/// Once [`Ics20`](crate::ibc_applications::Ics20) completes the execution of [`IbcApplication::packet_receive`], [`Middleware::mid_packet_receive_after`] in the
/// [`IbcHook`](crate::ibc_applications::IbcHook) is triggered, where the smart contract is triggered to send the tokens. This functionality is
/// managed in the `after` function as the [`Ics20`](crate::ibc_applications::Ics20) module must mint the tokens first.
pub trait Middleware {
    /// Return the inner [`IbcApplication`]
    fn get_inner(&self) -> &dyn IbcAndStargate;

    /// Function triggered before the calling of inner [`IbcApplication::handle_outgoing_packet`].
    ///
    /// If the return type is [`MiddlewareResponse::Continue(IbcMsg)`], the returned [`IbcMsg`] will forwarded to the inner [`IbcApplication::handle_outgoing_packet`].
    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]
    fn mid_handle_outgoing_packet(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        sender: Addr,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcMsg,
        channel: IbcChannelWrapper,
    ) -> AppResult<MiddlewareResponse<AppResponse, IbcMsg>> {
        Ok(MiddlewareResponse::Continue(msg))
    }

    /// Function triggered after [`IbcApplication::handle_outgoing_packet`] only if [`Middleware::mid_handle_outgoing_packet`] returned [`MiddlewareResponse::Continue`]
    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]
    fn mid_handle_outgoing_packet_after(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        sender: Addr,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        original_msg: IbcMsg,
        forwarded_msg: IbcMsg,
        returning_reponse: AppResponse,
        channel: IbcChannelWrapper,
    ) -> AppResult<AppResponse> {
        Ok(AppResponse::default())
    }

    /// Function triggered before the calling of inner [`IbcApplication::packet_receive`].
    ///
    /// If the return type is [`MiddlewareResponse::Continue(IbcPacketReceiveMsg)`], the returned [`IbcPacketReceiveMsg`] will forwarded to the inner [`IbcApplication::packet_receive`].
    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]
    fn mid_packet_receive_before(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        packet: IbcPacketReceiveMsg,
    ) -> InfallibleResult<
        MiddlewareResponse<PacketReceiveOk, IbcPacketReceiveMsg>,
        PacketReceiveFailing,
    > {
        InfallibleResult::Ok(MiddlewareResponse::Continue(packet))
    }

    /// Function triggered after [`IbcApplication::packet_receive`] only if [`Middleware::mid_packet_receive_before`] returned [`MiddlewareResponse::Continue`]
    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]
    fn mid_packet_receive_after(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        original_packet: IbcPacketReceiveMsg,
        forwarded_packet: IbcPacketReceiveMsg,
        returning_reponse: InfallibleResult<PacketReceiveOk, PacketReceiveFailing>,
    ) -> InfallibleResult<MidRecOk, MidRecFailing> {
        InfallibleResult::Ok(MidRecOk {
            response: AppResponse::default(),
            ack: AckSetting::UseChildren,
        })
    }

    /// Function triggered before the calling of inner [`IbcApplication::packet_ack`].
    ///
    /// If the return type is [`MiddlewareResponse::Continue(AckPacket)`], the returned [`AckPacket`] will forwarded to the inner [`IbcApplication::packet_ack`].
    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]

    fn mid_packet_ack_before(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        packet: AckPacket,
    ) -> AppResult<MiddlewareResponse<AppResponse, AckPacket>> {
        Ok(MiddlewareResponse::Continue(packet))
    }
    /// Function triggered after [`IbcApplication::packet_ack`] only if [`Middleware::mid_packet_ack_before`] returned [`MiddlewareResponse::Continue`]
    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]

    fn mid_packet_ack_after(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        original_packet: AckPacket,
        forwarded_packet: AckPacket,
        returning_reponse: AppResponse,
    ) -> AppResult<AppResponse> {
        Ok(AppResponse::default())
    }

    /// Function triggered before the calling of inner [`IbcApplication::packet_timeout`].
    ///
    /// If the return type is [`MiddlewareResponse::Continue(TimeoutPacket)`], the returned [`TimeoutPacket`] will forwarded to the inner [`IbcApplication::packet_timeout`].
    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]
    fn mid_packet_timeout_before(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        packet: TimeoutPacket,
    ) -> AppResult<MiddlewareResponse<AppResponse, TimeoutPacket>> {
        Ok(MiddlewareResponse::Continue(packet))
    }

    /// Function triggered after [`IbcApplication::packet_timeout`] only if [`Middleware::mid_packet_timeout_before`] returned [`MiddlewareResponse::Continue`]
    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]
    fn mid_packet_timeout_after(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        torage: Rc<RefCell<&mut dyn Storage>>,
        original_packet: TimeoutPacket,
        forwarded_packet: TimeoutPacket,
        returning_reponse: AppResponse,
    ) -> AppResult<AppResponse> {
        Ok(AppResponse::default())
    }

    /// Function triggered before the calling of inner [`IbcApplication::open_channel`].
    ///
    /// If the return type is [`MiddlewareResponse::Continue(IbcChannelOpenMsg)`], the returned [`IbcChannelOpenMsg`] will forwarded to the inner [`IbcApplication::open_channel`].
    #[allow(unused_variables)]
    fn mid_open_channel_before(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcChannelOpenMsg,
    ) -> AppResult<MiddlewareResponse<AppResponse, IbcChannelOpenMsg>> {
        Ok(MiddlewareResponse::Continue(msg))
    }

    /// Function triggered after [`IbcApplication::open_channel`] only if [`Middleware::mid_open_channel_before`] returned [`MiddlewareResponse::Continue`]
    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]
    fn mid_open_channel_after(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        original_msg: IbcChannelOpenMsg,
        forwarded_msg: IbcChannelOpenMsg,
        returning_reponse: AppResponse,
    ) -> AppResult<AppResponse> {
        Ok(AppResponse::default())
    }

    /// Function triggered before the calling of inner [`IbcApplication::channel_connect`].
    ///
    /// If the return type is [`MiddlewareResponse::Continue(IbcChannelConnectMsg)`], the returned [`IbcChannelConnectMsg`] will forwarded to the inner [`IbcApplication::channel_connect`].
    #[allow(unused_variables)]
    fn mid_channel_connect_before(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcChannelConnectMsg,
    ) -> AppResult<MiddlewareResponse<AppResponse, IbcChannelConnectMsg>> {
        Ok(MiddlewareResponse::Continue(msg))
    }

    /// Function triggered after [`IbcApplication::channel_connect`] only if [`Middleware::mid_channel_connect_before`] returned [`MiddlewareResponse::Continue`]
    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]
    fn mid_channel_connect_after(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        original_msg: IbcChannelConnectMsg,
        forwarded_msg: IbcChannelConnectMsg,
        returning_reponse: AppResponse,
    ) -> AppResult<AppResponse> {
        Ok(AppResponse::default())
    }
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
            MiddlewareResponse::Continue(next_msg) => {
                let sub_response = self.get_inner().handle_outgoing_packet(
                    api,
                    block,
                    sender.clone(),
                    router,
                    storage.clone(),
                    next_msg.clone(),
                    channel.clone(),
                )?;

                let res = self.mid_handle_outgoing_packet_after(
                    api,
                    block,
                    sender,
                    router,
                    storage,
                    msg,
                    next_msg,
                    sub_response.clone(),
                    channel,
                )?;

                Ok(res.merge(sub_response))
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
                        next_packet.clone(),
                    );

                    match self.mid_packet_receive_after(
                        api,
                        block,
                        router,
                        storage,
                        original_packet,
                        next_packet,
                        sub_response.clone(),
                    ) {
                        InfallibleResult::Ok(ok) => InfallibleResult::Ok(PacketReceiveOk {
                            response: ok.response.try_merge(sub_response.clone()),
                            ack: ok.ack.merge_ack(sub_response),
                        }),
                        InfallibleResult::Err(err) => InfallibleResult::Err(PacketReceiveFailing {
                            error: err.error,
                            ack: err.ack.merge_ack(sub_response),
                        }),
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
        msg: AckPacket,
    ) -> AppResult<AppResponse> {
        match self.mid_packet_ack_before(api, block, router, storage.clone(), msg.clone())? {
            MiddlewareResponse::Stop(response) => Ok(response),
            MiddlewareResponse::Continue(next_packet) => {
                let sub_response = self.get_inner().packet_ack(
                    api,
                    block,
                    router,
                    storage.clone(),
                    next_packet.clone(),
                )?;

                let res = self.mid_packet_ack_after(
                    api,
                    block,
                    router,
                    storage,
                    msg,
                    next_packet,
                    sub_response.clone(),
                )?;
                Ok(res.merge(sub_response))
            }
        }
    }

    fn packet_timeout(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: TimeoutPacket,
    ) -> AppResult<AppResponse> {
        match self.mid_packet_timeout_before(api, block, router, storage.clone(), msg.clone())? {
            MiddlewareResponse::Stop(response) => Ok(response),
            MiddlewareResponse::Continue(next_packet) => {
                let sub_response = self.get_inner().packet_timeout(
                    api,
                    block,
                    router,
                    storage.clone(),
                    msg.clone(),
                )?;

                let res = self.mid_packet_timeout_after(
                    api,
                    block,
                    router,
                    storage,
                    msg,
                    next_packet,
                    sub_response.clone(),
                )?;
                Ok(res.merge(sub_response))
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
        match self.mid_open_channel_before(api, block, router, storage.clone(), msg.clone())? {
            MiddlewareResponse::Stop(response) => Ok(response),
            MiddlewareResponse::Continue(next_msg) => {
                let sub_response = self.get_inner().open_channel(
                    api,
                    block,
                    router,
                    storage.clone(),
                    msg.clone(),
                )?;

                let res = self.mid_open_channel_after(
                    api,
                    block,
                    router,
                    storage,
                    msg,
                    next_msg,
                    sub_response.clone(),
                )?;

                Ok(res.merge(sub_response))
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
        match self.mid_channel_connect_before(api, block, router, storage.clone(), msg.clone())? {
            MiddlewareResponse::Stop(response) => Ok(response),
            MiddlewareResponse::Continue(next_msg) => {
                let sub_response = self.get_inner().channel_connect(
                    api,
                    block,
                    router,
                    storage.clone(),
                    msg.clone(),
                )?;
                let res = self.mid_channel_connect_after(
                    api,
                    block,
                    router,
                    storage,
                    msg,
                    next_msg,
                    sub_response.clone(),
                )?;

                Ok(res.merge(sub_response))
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

/// Define how the ack has to be handled
pub enum AckSetting {
    /// The returned value override the ack returned from inner application.
    Replace(Binary),
    /// Clear the ack returned from the inner application.
    Remove,
    /// Use the inner ack.
    UseChildren,
}

impl AckSetting {
    pub(crate) fn merge_ack(
        &self,
        response: InfallibleResult<PacketReceiveOk, PacketReceiveFailing>,
    ) -> Option<Binary> {
        let ack = match response {
            InfallibleResult::Ok(ok) => ok.ack,
            InfallibleResult::Err(err) => err.ack,
        };

        match self {
            AckSetting::Replace(replace) => Some(replace.clone()),
            AckSetting::Remove => None,
            AckSetting::UseChildren => ack,
        }
    }
}

/// [Middleware::mid_packet_receive_after] Ok Result type.
pub struct MidRecOk {
    /// Response, it will be merged with inner application Response.
    pub response: AppResponse,
    /// Specifiy how ack has to be managed.
    pub ack: AckSetting,
}

impl MidRecOk {
    /// Create a [`MidRecOk`] with [`MidRecOk::ack`] as [AckSetting::UseChildren]
    pub fn use_children(response: AppResponse) -> Self {
        Self {
            response,
            ack: AckSetting::UseChildren,
        }
    }
}

impl Default for MidRecOk {
    fn default() -> Self {
        Self {
            response: Default::default(),
            ack: AckSetting::UseChildren,
        }
    }
}

/// [Middleware::mid_packet_receive_after] Err Result type.
pub struct MidRecFailing {
    /// Stringed error.
    pub error: String,
    /// Specifiy how ack has to be managed.
    pub ack: AckSetting,
}

impl MidRecFailing {
    /// Constructor
    pub fn new(error: impl Into<String>, ack: Binary) -> Self {
        Self {
            error: error.into(),
            ack: AckSetting::Replace(ack),
        }
    }
}
