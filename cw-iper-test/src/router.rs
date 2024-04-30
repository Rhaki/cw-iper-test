use crate::error::AppResult;
use anyhow::{anyhow, bail};
use cosmwasm_std::{from_json, to_json_binary, Addr, Binary};
use cw_multi_test::{AppResponse, SudoMsg};
use serde::{de::DeserializeOwned, Serialize};

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

                    UseRouterResponse::QueryResponse {
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

                    UseRouterResponse::ExecCResponse { response: res }
                }
                UseRouter::Sudo { msg } => {
                    let res = $router.sudo($api, *$rc_storage.borrow_mut(), $block, msg)?;

                    UseRouterResponse::SudoResponse { response: res }
                }
                UseRouter::TryExec {
                    b64_msg,
                    sender_msg,
                } => {
                    match cw_multi_test::transactional(
                        *$rc_storage.borrow_mut(),
                        |write_cache, _| {
                            Ok($router.execute(
                                $api,
                                write_cache,
                                $block,
                                sender_msg,
                                from_json(b64_msg)?,
                            )?)
                        },
                    ) {
                        Ok(res) => UseRouterResponse::TryExecResponse(
                            crate::router::TryUseRouterResponse::Ok(res),
                        ),
                        Err(err) => UseRouterResponse::TryExecResponse(
                            crate::router::TryUseRouterResponse::Err(err.to_string()),
                        ),
                    }
                }

                UseRouter::TrySudo { msg } => {
                    match cw_multi_test::transactional(
                        *$rc_storage.borrow_mut(),
                        |write_cache, _| Ok($router.sudo($api, write_cache, $block, msg)?),
                    ) {
                        Ok(res) => UseRouterResponse::TrySudoResponse(
                            crate::router::TryUseRouterResponse::Ok(res),
                        ),
                        Err(err) => UseRouterResponse::TrySudoResponse(
                            crate::router::TryUseRouterResponse::Err(err.to_string()),
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
    QueryResponse { b64_response: Binary },
    ExecCResponse { response: AppResponse },
    SudoResponse { response: AppResponse },
    TryExecResponse(TryUseRouterResponse),
    TrySudoResponse(TryUseRouterResponse),
}

#[derive(Debug)]
pub enum TryUseRouterResponse {
    Ok(AppResponse),
    Err(String),
}

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
            UseRouterResponse::QueryResponse { b64_response } => {
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
            UseRouterResponse::ExecCResponse { response } => Ok(response),
            _ => Err(anyhow!("unexpected response")),
        }
    }

    pub fn sudo(&self, msg: SudoMsg) -> AppResult<AppResponse> {
        let res = (self.closure)(UseRouter::Sudo { msg })?;

        match res {
            UseRouterResponse::SudoResponse { response } => Ok(response),
            _ => Err(anyhow!("unexpected response")),
        }
    }

    /// Try to execute a `CosmosMsg`. If the execution fails, the state is not changed.
    pub fn try_execute<T: Serialize>(&self, sender: Addr, comsos_msg: T) -> TryUseRouterResponse {
        match (self.closure)(UseRouter::TryExec {
            sender_msg: sender,
            b64_msg: to_json_binary(&comsos_msg).unwrap(),
        }) {
            Ok(UseRouterResponse::TryExecResponse(res)) => res,
            _ => TryUseRouterResponse::Err("unexpected response".to_string()),
        }
    }

    /// Try to execute a `SudoMsg`. If the execution fails, the state is not changed.
    pub fn try_sudo(&self, msg: SudoMsg) -> TryUseRouterResponse {
        match (self.closure)(UseRouter::TrySudo { msg }) {
            Ok(UseRouterResponse::TrySudoResponse(res)) => res,
            _ => TryUseRouterResponse::Err("unexpected response".to_string()),
        }
    }
}
