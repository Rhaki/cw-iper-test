use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{
    Addr, Api, Binary, BlockInfo, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcMsg,
    IbcPacketReceiveMsg, Storage,
};
use cw_multi_test::{AppResponse, MockApiBech32};

use crate::{
    error::AppResult,
    ibc::IbcChannelWrapper,
    ibc_module::{AckPacket, TimeoutPacket},
    iper_app::InfallibleResult,
    router::RouterWrapper,
};

/// This trait identifies a generic `IBC application`(e.g., [`IC20`](crate::ibc_applications::Ics20), `ICA`, etc.) that is managed by the [`IperIbcModule`](crate::ibc_module::IperIbcModule).
/// The [`IperIbcModule`](crate::ibc_module::IperIbcModule) is a structure implementing both [`Ibc`](cw_multi_test::Ibc) and [`Module`](cw_multi_test::Module)
/// traits and serves as the `IBC module` for the [`App`](cw_multi_test::App) class of an [`IperApp`](crate::IperApp).
///
/// `IperIbcModule` will invoke a function implemented by this trait under the following conditions:
///
/// - **handle_outgoing_packet**: An `IBC packet` is emitted and the source `channel-id` is this [`IbcApplication`].
/// - **packet_receive**: An `IBC packet` is received and the destination `channel-id` is this [`IbcApplication`].
/// - **packet_ack**: An `acknowledgment packet` returns and the source `channel-id` was this [`IbcApplication`].
/// - **packet_timeout**: A `timeout packet` returns and the source `channel-id` was this [`IbcApplication`].
/// - **open_channel**: An `IBC channel` is being opened that carries this [`IbcApplication`].
/// - **channel_connect**: An `IBC channel` is being connected that carries this [`IbcApplication`].
///
/// ## Implementation of the trait:
/// In order to be implemented, the struct has to implement also [`IbcPortInterface`]
///
/// Use the derive macro `IbcPort` `from cw-iper-test-macros`
/// ## Example:
/// ```ignore
/// #[derive(IbcPort)]
/// #[ibc_port = "transfer"]
/// pub struct Ics20;
pub trait IbcApplication: IbcPortInterface {
    /// An `IBC packet` is emitted and the source `channel-id` is this [`IbcApplication`].
    ///
    /// For example. on [`Ics20`](crate::ibc_applications::Ics20) the transfer amount has to be burned/locked from the sender address
    #[allow(clippy::too_many_arguments)]

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

    /// An `IBC packet` is received and the destination `channel-id` is this [[`IbcApplication`]].
    ///
    /// The return type is [`InfallibleResult`], allowing to revert any Storage changes without raising errors.
    fn packet_receive(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcPacketReceiveMsg,
    ) -> InfallibleResult<PacketReceiveOk, PacketReceiveFailing>;

    /// An `acknowledgment packet` returns and the source `channel-id` was this [`IbcApplication`].
    fn packet_ack(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: AckPacket,
    ) -> AppResult<AppResponse>;
    /// A `timeout packet` returns and the source `channel-id` was this [`IbcApplication`].
    fn packet_timeout(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: TimeoutPacket,
    ) -> AppResult<AppResponse>;

    /// An `IBC channel` is being opened that carries this [`IbcApplication`].
    fn open_channel(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcChannelOpenMsg,
    ) -> AppResult<AppResponse>;

    /// An `IBC channel` is being connected that carries this [`IbcApplication`].
    fn channel_connect(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcChannelConnectMsg,
    ) -> AppResult<AppResponse>;

    ///
    fn init(&self, api: &MockApiBech32, storage: &mut dyn Storage);
}

/// This trait is used to implement the `port_name` for a [`IbcApplication`].
///
/// For implement this trait, use the derive macro `IbcPort` `from cw-iper-test-macros`
/// ## Example:
/// ```ignore
/// #[derive(IbcPort)]
/// #[ibc_port = "transfer"]
/// pub struct Ics20;
pub trait IbcPortInterface {
    /// return the `port` name of an [`IbcApplication`]
    fn port_name(&self) -> String;
}

/// `Ok` [`InfallibleResult`] from [`IbcApplication::packet_receive`].
///
/// if [`PacketReceiveOk::ack`] is [`Some`], an [`IbcPacketType::AckPacket`](crate::ibc_module::IbcPacketType) is emitted and ready to be `relayed`
#[derive(Debug, Clone)]
pub struct PacketReceiveOk {
    /// AppResponse of the [`IbcApplication::packet_receive`] execution.
    ///
    ///  When the packet will be relayed from [`Ecosystem`](crate::ecosystem::Ecosystem) this error text will be returned
    pub response: AppResponse,
    /// if [`Some`], an [`IbcPacketType::AckPacket`](crate::ibc_module::IbcPacketType) is emitted and ready to be `relayed`
    pub ack: Option<Binary>,
}

/// `Err` [`InfallibleResult`] from [`IbcApplication::packet_receive`].
/// Any changes on the `Storage` emitted during [`IbcApplication::packet_receive`] will be reverted.
///
/// if [`PacketReceiveOk::ack`] is [`Some`], an [`IbcPacketType::AckPacket`](crate::ibc_module::IbcPacketType) is emitted and ready to be `relayed`
#[derive(Debug, Clone)]
pub struct PacketReceiveFailing {
    /// Stringed error. When the packet will be relayed from [`Ecosystem`](crate::ecosystem::Ecosystem) this error text will be returned
    pub error: String,
    /// if [`Some`], an [`IbcPacketType::AckPacket`](crate::ibc_module::IbcPacketType) is emitted and ready to be `relayed`
    pub ack: Option<Binary>,
}
