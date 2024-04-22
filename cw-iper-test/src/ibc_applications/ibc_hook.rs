use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{
    from_json, to_json_binary, Addr, Api, BlockInfo, IbcChannelConnectMsg, IbcChannelOpenMsg,
    IbcMsg, IbcPacket, IbcPacketAckMsg, IbcPacketReceiveMsg, Storage,
};
use cw_multi_test::AppResponse;
use ibc_proto::ibc::apps::transfer::v2::FungibleTokenPacketData;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    error::AppResult,
    ibc::IbcChannelWrapper,
    ibc_application::{IbcApplication, PacketReceiveResponse},
    router::RouterWrapper,
};

use super::middleware::{
    IbcAndStargate, Middleware, MiddlewareResponse, MiddlewareUniqueResponse, PacketToNext,
};

pub struct IbcHook {
    pub inner: Box<dyn IbcApplication>,
}

impl IbcHook {
    pub fn new<T: IbcApplication + 'static>(inner: T) -> Self {
        Self {
            inner: Box::new(inner),
        }
    }
}

impl Middleware for IbcHook {
    fn get_inner(&self) -> &dyn IbcAndStargate {
        todo!()
    }

    fn mid_handle_outgoing_packet(
        &self,
        _api: &dyn Api,
        _block: &BlockInfo,
        _sender: Addr,
        _router: &RouterWrapper,
        _storage: Rc<RefCell<&mut dyn Storage>>,
        _msg: IbcMsg,
        _channel: IbcChannelWrapper,
    ) -> AppResult<MiddlewareUniqueResponse<AppResponse>> {
        Ok(MiddlewareResponse::Continue(AppResponse::default()))
    }

    fn mid_packet_receive_before(
        &self,
        api: &dyn Api,
        _block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        packet: IbcPacketReceiveMsg,
    ) -> AppResult<MiddlewareResponse<PacketReceiveResponse, PacketToNext>> {
        let clos = || -> AppResult<MiddlewareResponse<PacketReceiveResponse, PacketToNext>> {
            let mut data: FungibleTokenPacketData = from_json(&packet.packet.data)?;
            if data.memo != "".to_string() {
                serde_json::from_str::<MemoField<WasmField>>(&data.memo)?;

                // Create ibc_hook_sender address;
                let ibc_hook_sender = Addr::unchecked("addr");

                data.receiver = ibc_hook_sender.to_string();

                let forwarded_packet = IbcPacketReceiveMsg::new(
                    IbcPacket::new(
                        to_json_binary(&data)?,
                        packet.packet.src.clone(),
                        packet.packet.dest.clone(),
                        packet.packet.sequence,
                        packet.packet.timeout.clone(),
                    ),
                    packet.relayer.clone(),
                );

                Ok(MiddlewareResponse::Continue(PacketToNext {
                    packet: forwarded_packet,
                }))
            } else {
                Ok(MiddlewareResponse::Continue(PacketToNext {
                    packet: packet.clone(),
                }))
            }
        };

        match clos() {
            Ok(response) => Ok(response),
            Err(err) => Ok(MiddlewareResponse::Continue(PacketToNext { packet })),
        }
    }

    fn mid_packet_receive_after(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        original_packet: IbcPacketReceiveMsg,
        forwarded_packet: IbcPacketReceiveMsg,
        forwarded_response: PacketReceiveResponse,
    ) -> AppResult<PacketReceiveResponse> {
        todo!()
    }

    fn mid_packet_ack(
        &self,
        _api: &dyn Api,
        _block: &BlockInfo,
        _router: &RouterWrapper,
        _storage: Rc<RefCell<&mut dyn Storage>>,
        _msg: IbcPacketAckMsg,
    ) -> AppResult<MiddlewareUniqueResponse<AppResponse>> {
        todo!()
    }

    fn mid_open_channel(
        &self,
        _api: &dyn Api,
        _block: &BlockInfo,
        _router: &RouterWrapper,
        _storage: Rc<RefCell<&mut dyn Storage>>,
        _msg: IbcChannelOpenMsg,
    ) -> AppResult<MiddlewareUniqueResponse<AppResponse>> {
        Ok(MiddlewareResponse::Continue(AppResponse::default()))
    }

    fn mid_channel_connect(
        &self,
        _api: &dyn Api,
        _block: &BlockInfo,
        _router: &RouterWrapper,
        _storage: Rc<RefCell<&mut dyn Storage>>,
        _msg: IbcChannelConnectMsg,
    ) -> AppResult<MiddlewareUniqueResponse<AppResponse>> {
        Ok(MiddlewareResponse::Continue(AppResponse::default()))
    }
}

#[derive(Serialize, Deserialize)]
pub struct MemoField<T> {
    pub memo: T,
}

#[derive(Serialize, Deserialize)]
pub struct WasmField {
    contract: String,
    msg: Value,
}
