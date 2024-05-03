use anyhow::anyhow;
use cosmwasm_std::{
    CustomMsg, CustomQuery, DepsMut, Empty, Env, Ibc3ChannelOpenResponse, IbcBasicResponse,
    IbcChannelCloseMsg, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcPacketAckMsg,
    IbcPacketReceiveMsg, IbcPacketTimeoutMsg, IbcReceiveResponse, Never,
};
use cw_multi_test::{Contract, ContractWrapper};
use serde::de::DeserializeOwned;
use std::fmt::{Debug, Display};

use crate::error::AppResult;

use self::closures::{
    IbcChannelCloseClosure, IbcChannelCloseFn, IbcChannelConnectClosure, IbcChannelConnectFn,
    IbcChannelOpenClosure, IbcChannelOpenFn, IbcPacketAckClosure, IbcPacketAckFn,
    IbcPacketReceiveClosure, IbcPacketReceiveFn, IbcPacketTimeoutClosure, IbcPacketTimeoutFn,
};

use cw_multi_test::error::AnyError;

#[rustfmt::skip]
mod closures {
    use cosmwasm_std::{Ibc3ChannelOpenResponse, IbcReceiveResponse};
    use super::*;

    pub type IbcChannelOpenFn<E, Q>       = fn(DepsMut<Q>, Env, IbcChannelOpenMsg)    -> Result<Option<Ibc3ChannelOpenResponse>, E>;
    pub type IbcChannelCloseFn<E, Q, C>   = fn(DepsMut<Q>, Env, IbcChannelCloseMsg)   -> Result<IbcBasicResponse<C>, E>;
    pub type IbcChannelConnectFn<E, Q, C> = fn(DepsMut<Q>, Env, IbcChannelConnectMsg) -> Result<IbcBasicResponse<C>, E>;
    pub type IbcPacketReceiveFn<Q, C>     = fn(DepsMut<Q>, Env, IbcPacketReceiveMsg)  -> Result<IbcReceiveResponse<C>, Never>;
    pub type IbcPacketAckFn<E, Q, C>      = fn(DepsMut<Q>, Env, IbcPacketAckMsg)      -> Result<IbcBasicResponse<C>, E>;
    pub type IbcPacketTimeoutFn<E, Q, C>  = fn(DepsMut<Q>, Env, IbcPacketTimeoutMsg)  -> Result<IbcBasicResponse<C>, E>;
    
    pub type IbcChannelOpenClosure<E, Q>       = Box<dyn Fn(DepsMut<Q>, Env, IbcChannelOpenMsg)    -> Result<Option<Ibc3ChannelOpenResponse>, E>>;
    pub type IbcChannelCloseClosure<E, Q, C>   = Box<dyn Fn(DepsMut<Q>, Env, IbcChannelCloseMsg)   -> Result<IbcBasicResponse<C>, E>>;
    pub type IbcChannelConnectClosure<E, Q, C> = Box<dyn Fn(DepsMut<Q>, Env, IbcChannelConnectMsg) -> Result<IbcBasicResponse<C>, E>>;
    pub type IbcPacketReceiveClosure<Q, C>     = Box<dyn Fn(DepsMut<Q>, Env, IbcPacketReceiveMsg)  -> Result<IbcReceiveResponse<C>, Never>>;
    pub type IbcPacketAckClosure<E, Q, C>      = Box<dyn Fn(DepsMut<Q>, Env, IbcPacketAckMsg)      -> Result<IbcBasicResponse<C>, E>>;
    pub type IbcPacketTimeoutClosure<E, Q, C>  = Box<dyn Fn(DepsMut<Q>, Env, IbcPacketTimeoutMsg)  -> Result<IbcBasicResponse<C>, E>>;
}

/// Structure containing the various `ibc closures`
pub struct IbcClosures<E1, E2, E3, E4, E5, C, Q = Empty>
where
    Q: CustomQuery,
    C: CustomMsg,
{
    /// `#[entry_point]` `ibc_channel_open` closure
    pub fn_ibc_channel_open: IbcChannelOpenClosure<E1, Q>,
    /// `#[entry_point]` `ibc_channel_close` closure
    pub fn_ibc_channel_close: IbcChannelCloseClosure<E2, Q, C>,
    /// `#[entry_point]` `ibc_channel_connect` closure
    pub fn_ibc_channel_connect: IbcChannelConnectClosure<E3, Q, C>,
    /// `#[entry_point]` `ibc_packet_receive` closure
    pub fn_ibc_packet_receive: IbcPacketReceiveClosure<Q, C>,
    /// `#[entry_point]` `ibc_packet_ack` closure
    pub fn_ibc_packet_ack: IbcPacketAckClosure<E4, Q, C>,
    /// `#[entry_point]` `ibc_packet_timeout` closure
    pub fn_ibc_packet_timeout: IbcPacketTimeoutClosure<E5, Q, C>,
}

impl<E1, E2, E3, E4, E5, C, Q> IbcClosures<E1, E2, E3, E4, E5, C, Q>
where
    Q: CustomQuery + 'static,
    C: CustomMsg + 'static,
    E1: Display + Debug + Send + Sync + 'static,
    E2: Display + Debug + Send + Sync + 'static,
    E3: Display + Debug + Send + Sync + 'static,
    E4: Display + Debug + Send + Sync + 'static,
    E5: Display + Debug + Send + Sync + 'static,
{
    /// Constructor function
    pub fn new(
        fn_ibc_channel_open: IbcChannelOpenFn<E1, Q>,
        fn_ibc_channel_close: IbcChannelCloseFn<E2, Q, C>,
        fn_ibc_channel_connect: IbcChannelConnectFn<E3, Q, C>,
        fn_ibc_packet_receive: IbcPacketReceiveFn<Q, C>,
        fn_ibc_packet_ack: IbcPacketAckFn<E4, Q, C>,
        fn_ibc_packet_timeout: IbcPacketTimeoutFn<E5, Q, C>,
    ) -> IbcClosures<E1, E2, E3, E4, E5, C, Q> {
        IbcClosures {
            fn_ibc_channel_open: Box::new(fn_ibc_channel_open),
            fn_ibc_channel_close: Box::new(fn_ibc_channel_close),
            fn_ibc_channel_connect: Box::new(fn_ibc_channel_connect),
            fn_ibc_packet_receive: Box::new(fn_ibc_packet_receive),
            fn_ibc_packet_ack: Box::new(fn_ibc_packet_ack),
            fn_ibc_packet_timeout: Box::new(fn_ibc_packet_timeout),
        }
    }

    /// Create a new [`IbcClosures`] as [`IbcContract`]
    pub fn new_as_ibc_contract(
        fn_ibc_channel_open: IbcChannelOpenFn<E1, Q>,
        fn_ibc_channel_close: IbcChannelCloseFn<E2, Q, C>,
        fn_ibc_channel_connect: IbcChannelConnectFn<E3, Q, C>,
        fn_ibc_packet_receive: IbcPacketReceiveFn<Q, C>,
        fn_ibc_packet_ack: IbcPacketAckFn<E4, Q, C>,
        fn_ibc_packet_timeout: IbcPacketTimeoutFn<E5, Q, C>,
    ) -> Box<dyn IbcContract<C, Q>> {
        Box::new(IbcClosures {
            fn_ibc_channel_open: Box::new(fn_ibc_channel_open),
            fn_ibc_channel_close: Box::new(fn_ibc_channel_close),
            fn_ibc_channel_connect: Box::new(fn_ibc_channel_connect),
            fn_ibc_packet_receive: Box::new(fn_ibc_packet_receive),
            fn_ibc_packet_ack: Box::new(fn_ibc_packet_ack),
            fn_ibc_packet_timeout: Box::new(fn_ibc_packet_timeout),
        }) as Box<dyn IbcContract<C, Q>>
    }
}

/// Wrapper struct to store both default [`Contract`] trait and optional [`IbcContract`] trait
pub struct IperContract<C, Q = Empty>
where
    C: CustomMsg,
    Q: CustomQuery,
{
    /// default [`Contract`] interface
    pub base: Box<dyn Contract<C, Q>>,
    /// Optional [`IbcContract`] interface. Used only on `contracts` that implements `ibc entry_points`
    pub ibc: Option<Box<dyn IbcContract<C, Q>>>,
}

impl<C, Q> IperContract<C, Q>
where
    C: CustomMsg,
    Q: CustomQuery,
{
    /// Constructor function
    pub fn new(
        base: Box<dyn Contract<C, Q>>,
        ibc: Option<Box<dyn IbcContract<C, Q>>>,
    ) -> IperContract<C, Q> {
        Self { base, ibc }
    }
}

/// Similar to [`Contract`], this trait serves as a primary interface for interacting `ibc entry_points` functions.
pub trait IbcContract<C, Q = Empty>
where
    C: CustomMsg,
    Q: CustomQuery,
{
    /// Evaluates contract's `ibc_channel_open` `entry_point`.
    fn ibc_channel_open(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: IbcChannelOpenMsg,
    ) -> AppResult<Option<Ibc3ChannelOpenResponse>>;

    /// Evaluates contract's `ibc_channel_close` `entry_point`.
    fn ibc_channel_close(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: IbcChannelCloseMsg,
    ) -> AppResult<IbcBasicResponse<C>>;

    /// Evaluates contract's `ibc_channel_connect` `entry_point`.
    fn ibc_channel_connect(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: IbcChannelConnectMsg,
    ) -> AppResult<IbcBasicResponse<C>>;

    /// Evaluates contract's `ibc_packet_receive` `entry_point`.
    fn ibc_packet_receive(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: IbcPacketReceiveMsg,
    ) -> AppResult<IbcReceiveResponse<C>>;

    /// Evaluates contract's `ibc_packet_ack` `entry_point`.
    fn ibc_packet_ack(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: IbcPacketAckMsg,
    ) -> AppResult<IbcBasicResponse<C>>;

    /// Evaluates contract's `ibc_packet_timeout` `entry_point`.
    fn ibc_packet_timeout(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: IbcPacketTimeoutMsg,
    ) -> AppResult<IbcBasicResponse<C>>;
}

impl<E1, E2, E3, E4, E5, C: CustomMsg, Q: CustomQuery> IbcContract<C, Q>
    for IbcClosures<E1, E2, E3, E4, E5, C, Q>
where
    E1: Display + Debug + Send + Sync + 'static, // Type of error returned from `execute` entry-point.
    E2: Display + Debug + Send + Sync + 'static, // Type of error returned from `instantiate` entry-point.
    E3: Display + Debug + Send + Sync + 'static, // Type of error returned from `query` entry-point.
    E4: Display + Debug + Send + Sync + 'static, // Type of error returned from `sudo` entry-point.
    E5: Display + Debug + Send + Sync + 'static, // Type of error returned from `reply` entry-point.
{
    fn ibc_channel_open(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: IbcChannelOpenMsg,
    ) -> AppResult<Option<Ibc3ChannelOpenResponse>> {
        (self.fn_ibc_channel_open)(deps, env, msg).map_err(|err| anyhow!(err))
    }

    fn ibc_channel_close(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: IbcChannelCloseMsg,
    ) -> AppResult<IbcBasicResponse<C>> {
        (self.fn_ibc_channel_close)(deps, env, msg).map_err(|err| anyhow!(err))
    }

    fn ibc_channel_connect(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: IbcChannelConnectMsg,
    ) -> AppResult<IbcBasicResponse<C>> {
        (self.fn_ibc_channel_connect)(deps, env, msg).map_err(|err| anyhow!(err))
    }

    fn ibc_packet_receive(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: IbcPacketReceiveMsg,
    ) -> AppResult<IbcReceiveResponse<C>> {
        (self.fn_ibc_packet_receive)(deps, env, msg).map_err(|err| anyhow!(err))
    }

    fn ibc_packet_ack(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: IbcPacketAckMsg,
    ) -> AppResult<IbcBasicResponse<C>> {
        (self.fn_ibc_packet_ack)(deps, env, msg).map_err(|err| anyhow!(err))
    }

    fn ibc_packet_timeout(
        &self,
        deps: DepsMut<Q>,
        env: Env,
        msg: IbcPacketTimeoutMsg,
    ) -> AppResult<IbcBasicResponse<C>> {
        (self.fn_ibc_packet_timeout)(deps, env, msg).map_err(|err| anyhow!(err))
    }
}

/// Extension of [`ContractWrapper`], allowing to transform it into [`Contract`] directly
pub trait ContractWrapperExt<
    T1,
    T2,
    T3,
    E1,
    E2,
    E3,
    C = Empty,
    Q = Empty,
    T4 = Empty,
    E4 = AnyError,
    E5 = AnyError,
    T6 = Empty,
    E6 = AnyError,
> where
    T1: DeserializeOwned, // Type of message passed to `execute` entry-point.
    T2: DeserializeOwned, // Type of message passed to `instantiate` entry-point.
    T3: DeserializeOwned, // Type of message passed to `query` entry-point.
    T4: DeserializeOwned, // Type of message passed to `sudo` entry-point.
    T6: DeserializeOwned, // Type of message passed to `migrate` entry-point.
    E1: Display + Debug + Send + Sync, // Type of error returned from `execute` entry-point.
    E2: Display + Debug + Send + Sync, // Type of error returned from `instantiate` entry-point.
    E3: Display + Debug + Send + Sync, // Type of error returned from `query` entry-point.
    E4: Display + Debug + Send + Sync, // Type of error returned from `sudo` entry-point.
    E5: Display + Debug + Send + Sync, // Type of error returned from `reply` entry-point.
    E6: Display + Debug + Send + Sync, // Type of error returned from `migrate` entry-point.
    C: CustomMsg,         // Type of custom message returned from all entry-points except `query`.
    Q: CustomQuery + DeserializeOwned,
{
    /// Transform [`ContractWrapper`] into [`Contract`]
    fn to_contract(self) -> Box<dyn Contract<C, Q>>;
}

impl<T1, T2, T3, E1, E2, E3, C, Q, T4, E4, E5, T6, E6>
    ContractWrapperExt<T1, T2, T3, E1, E2, E3, C, Q, T4, E4, E5, T6, E6>
    for ContractWrapper<T1, T2, T3, E1, E2, E3, C, Q, T4, E4, E5, T6, E6>
where
    T1: DeserializeOwned + 'static, // Type of message passed to `execute` entry-point.
    T2: DeserializeOwned + 'static, // Type of message passed to `instantiate` entry-point.
    T3: DeserializeOwned + 'static, // Type of message passed to `query` entry-point.
    T4: DeserializeOwned + 'static, // Type of message passed to `sudo` entry-point.
    T6: DeserializeOwned + 'static, // Type of message passed to `migrate` entry-point.
    E1: Display + Debug + Send + Sync + 'static, // Type of error returned from `execute` entry-point.
    E2: Display + Debug + Send + Sync + 'static, // Type of error returned from `instantiate` entry-point.
    E3: Display + Debug + Send + Sync + 'static, // Type of error returned from `query` entry-point.
    E4: Display + Debug + Send + Sync + 'static, // Type of error returned from `sudo` entry-point.
    E5: Display + Debug + Send + Sync + 'static, // Type of error returned from `reply` entry-point.
    E6: Display + Debug + Send + Sync + 'static, // Type of error returned from `migrate` entry-point.
    C: CustomMsg + 'static, // Type of custom message returned from all entry-points except `query`.
    Q: CustomQuery + DeserializeOwned + 'static, // Type of custom query in querier passed as deps/deps_mut to all entry-points.
{
    fn to_contract(self) -> Box<dyn Contract<C, Q>> {
        Box::new(self) as Box<dyn Contract<C, Q>>
    }
}
