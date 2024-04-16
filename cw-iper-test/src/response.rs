use cosmwasm_std::{
    to_json_binary, Binary, Ibc3ChannelOpenResponse, IbcBasicResponse, IbcReceiveResponse, Response,
};
use cw_multi_test::AppResponse;

use crate::{error::AppResult, ibc_module::IbcPacketType};

#[derive(Debug, Clone)]
pub struct RelayedResponse {
    pub packet: IbcPacketType,
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

pub trait AppResponseExt {
    fn merge(self, with: AppResponse) -> AppResponse;
}

impl AppResponseExt for AppResponse {
    fn merge(self, with: AppResponse) -> AppResponse {
        let mut base = self;

        let mut with = with;

        base.events.append(&mut with.events);

        if base.data.is_none() {
            base.data = with.data;
        }

        base
    }
}
