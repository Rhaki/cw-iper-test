use cosmwasm_std::{
    to_json_binary, Addr, Binary, Ibc3ChannelOpenResponse, IbcBasicResponse, IbcMsg,
    IbcReceiveResponse, Response,
};
use cw_multi_test::AppResponse;

use crate::error::AppResult;

#[derive(Debug, Clone)]
pub struct RelayedResponse {
    pub relayer: Addr,
    pub msg: IbcMsg,
    pub dest_response: AppResponse,
    pub ack: Option<Binary>,
    pub src_response: Option<AppResponse>,
}

pub trait IntoResponse<T> {
    fn into_app_response(self) -> AppResult<Response<T>>;
}

impl<T> IntoResponse<T> for AppResult<Option<Ibc3ChannelOpenResponse>> {
    fn into_app_response(self) -> AppResult<Response<T>> {
        Ok(Response::new().set_data(
            self?
                .map(|val| to_json_binary(&val))
                .transpose()?
                .unwrap_or_default(),
        ))
    }
}

impl<T> IntoResponse<T> for AppResult<IbcBasicResponse<T>> {
    fn into_app_response(self) -> AppResult<Response<T>> {
        let res = self?;

        Ok(Response::<T>::new()
            .add_submessages(res.messages)
            .add_attributes(res.attributes)
            .add_events(res.events))
    }
}

impl<T> IntoResponse<T> for AppResult<IbcReceiveResponse<T>> {
    fn into_app_response(self) -> AppResult<Response<T>> {
        let res = self?;

        let mut ret = Response::<T>::new()
            .add_submessages(res.messages)
            .add_attributes(res.attributes)
            .add_events(res.events);

        if let Some(data) = res.acknowledgement {
            ret = ret.set_data(data);
        }

        Ok(ret)
    }
}
