use crate::error::AppResult;
use anyhow::{anyhow, bail};
use cosmwasm_std::{from_json, to_json_binary, Addr, Binary};
use cw_multi_test::{AppResponse, SudoMsg};
use serde::{de::DeserializeOwned, Serialize};

/// Router closure
#[macro_export]
macro_rules! router_closure {
    ($router:expr, $api:expr, $rc_storage:expr, $block:expr) => {
        |action: UseRouter| {
            let res = match action {
                UseRouter::Query { b64_request } => {
                    let res = $router.query(
                        $api,
                        *$rc_storage.borrow(),
                        $block,
                        from_json(b64_request)?,
                    )?;

                    UseRouterResponse::Query {
                        b64_response: res.into(),
                    }
                }
                UseRouter::Exec {
                    b64_msg,
                    sender_msg,
                } => {
                    let res = $router.execute(
                        $api,
                        *$rc_storage.borrow_mut(),
                        $block,
                        sender_msg,
                        from_json(b64_msg)?,
                    )?;

                    UseRouterResponse::Exec { response: res }
                }
                UseRouter::Sudo { msg } => {
                    let res = $router.sudo($api, *$rc_storage.borrow_mut(), $block, msg)?;

                    UseRouterResponse::Sudo { response: res }
                }
                UseRouter::TryExec {
                    b64_msg,
                    sender_msg,
                } => {
                    match cw_multi_test::transactional(
                        *$rc_storage.borrow_mut(),
                        |write_cache, _| {
                            $router.execute(
                                $api,
                                write_cache,
                                $block,
                                sender_msg,
                                from_json(b64_msg)?,
                            )
                        },
                    ) {
                        Ok(res) => UseRouterResponse::TryExec(
                            $crate::router::TryUseRouterResponse::Ok(res),
                        ),
                        Err(err) => UseRouterResponse::TryExec(
                            $crate::router::TryUseRouterResponse::Err(err.to_string()),
                        ),
                    }
                }

                UseRouter::TrySudo { msg } => {
                    match cw_multi_test::transactional(
                        *$rc_storage.borrow_mut(),
                        |write_cache, _| $router.sudo($api, write_cache, $block, msg),
                    ) {
                        Ok(res) => UseRouterResponse::TrySudo(
                            $crate::router::TryUseRouterResponse::Ok(res),
                        ),
                        Err(err) => UseRouterResponse::TrySudo(
                            $crate::router::TryUseRouterResponse::Err(err.to_string()),
                        ),
                    }
                }
            };

            Ok(res)
        }
    };
}

pub enum UseRouter {
    Query { b64_request: Binary },
    Exec { b64_msg: Binary, sender_msg: Addr },
    Sudo { msg: SudoMsg },
    TryExec { b64_msg: Binary, sender_msg: Addr },
    TrySudo { msg: SudoMsg },
}

pub enum UseRouterResponse {
    Query { b64_response: Binary },
    Exec { response: AppResponse },
    Sudo { response: AppResponse },
    TryExec(TryUseRouterResponse),
    TrySudo(TryUseRouterResponse),
}

#[derive(Debug)]
pub enum TryUseRouterResponse {
    Ok(AppResponse),
    Err(String),
}

/// Alternative version of [`CosmosRouter`](cw_multi_test::CosmosRouter) interface.
/// 
/// Inside the 
/// [`IbcApplication`](crate::ibc_application::IbcApplication) and [`StargateApplication`](crate::stargate::StargateApplication),
/// this version of [`CosmosRouter`](cw_multi_test::CosmosRouter) is used because both
/// [`IbcApplication`](crate::ibc_application::IbcApplication) and [`StargateApplication`](crate::stargate::StargateApplication)
/// needs to be vtable compatible.
/// 
/// Passing the default [`CosmosRouter`](cw_multi_test::CosmosRouter) as argument of a function require to implements two generic type like [`Module::execute`](cw_multi_test::Module).
/// Since [`IbcApplication`](crate::ibc_application::IbcApplication) and [`StargateApplication`](crate::stargate::StargateApplication),
/// need to be vtable compatible, the default [`CosmosRouter`](cw_multi_test::CosmosRouter) cannot be used.
/// 
/// ```ignore
/// impl Module for IperStargateModule {
///     type ExecT = AnyMsg;
///     type QueryT = GrpcQuery;
///     type SudoT = Empty;
/// 
///     // <ExecC, QueryC> lead to Module to be not vtable compatible
///     fn execute<ExecC, QueryC>(
///         &self,
///         api: &dyn Api,
///         storage: &mut dyn Storage,
///         router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
///         block: &BlockInfo,
///         sender: Addr,
///         msg: Self::ExecT,
///     ) -> AppResult<AppResponse>
///     where
///         ExecC: CustomMsg + DeserializeOwned + 'static,
///         QueryC: CustomQuery + DeserializeOwned + 'static,
///     {
///         ...
///     }
/// ```
/// 
/// 
/// 
pub struct RouterWrapper<'a> {
    closure: &'a dyn Fn(UseRouter) -> AppResult<UseRouterResponse>,
}

impl<'a> RouterWrapper<'a> {
    pub fn new(closure: &'a dyn Fn(UseRouter) -> AppResult<UseRouterResponse>) -> Self {
        Self { closure }
    }

    pub fn query<T: Serialize, R: DeserializeOwned>(&self, query: T) -> AppResult<R> {
        let res = (self.closure)(UseRouter::Query {
            b64_request: to_json_binary(&query).unwrap(),
        })?;

        match res {
            UseRouterResponse::Query { b64_response } => {
                from_json(b64_response).map_err(|err| anyhow!(err))
            }
            _ => bail!("unexpected response"),
        }
    }

    pub fn execute<T: Serialize>(&self, sender: Addr, comsos_msg: T) -> AppResult<AppResponse> {
        let res = (self.closure)(UseRouter::Exec {
            sender_msg: sender,
            b64_msg: to_json_binary(&comsos_msg).unwrap(),
        })?;

        match res {
            UseRouterResponse::Exec { response } => Ok(response),
            _ => Err(anyhow!("unexpected response")),
        }
    }

    pub fn sudo(&self, msg: SudoMsg) -> AppResult<AppResponse> {
        let res = (self.closure)(UseRouter::Sudo { msg })?;

        match res {
            UseRouterResponse::Sudo { response } => Ok(response),
            _ => Err(anyhow!("unexpected response")),
        }
    }

    /// Try to execute a `CosmosMsg`. If the execution fails, the state is not changed.
    pub fn try_execute<T: Serialize>(&self, sender: Addr, comsos_msg: T) -> TryUseRouterResponse {
        match (self.closure)(UseRouter::TryExec {
            sender_msg: sender,
            b64_msg: to_json_binary(&comsos_msg).unwrap(),
        }) {
            Ok(UseRouterResponse::TryExec(res)) => res,
            _ => TryUseRouterResponse::Err("unexpected response".to_string()),
        }
    }

    /// Try to execute a `SudoMsg`. If the execution fails, the state is not changed.
    pub fn try_sudo(&self, msg: SudoMsg) -> TryUseRouterResponse {
        match (self.closure)(UseRouter::TrySudo { msg }) {
            Ok(UseRouterResponse::TrySudo(res)) => res,
            _ => TryUseRouterResponse::Err("unexpected response".to_string()),
        }
    }
}
