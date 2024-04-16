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
                UseRouter::ExecC {
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
            };

            Ok(res)
        }
    };
}

pub enum UseRouter {
    Query { b64_request: Binary },
    ExecC { b64_msg: Binary, sender_msg: Addr },
    Sudo { msg: SudoMsg },
}

pub enum UseRouterResponse {
    QueryResponse { b64_response: Binary },
    ExecCResponse { response: AppResponse },
    SudoResponse { response: AppResponse },
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
        let res = (self.closure)(UseRouter::ExecC {
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
}
