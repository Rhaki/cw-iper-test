use anyhow::bail;

use bech32::{encode as bech32_encode, Bech32, Hrp};

use cosmwasm_std::{Coin, Empty, Uint128};

use std::str::FromStr;
use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{
    from_json, to_json_binary, Addr, Api, BlockInfo, CosmosMsg, IbcChannelConnectMsg,
    IbcChannelOpenMsg, IbcMsg, IbcPacket, IbcPacketAckMsg, IbcPacketReceiveMsg, Storage, WasmMsg,
};
use cw_multi_test::AppResponse;
use ibc_proto::ibc::apps::transfer::v2::FungibleTokenPacketData;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::ibc_app::InfallibleResult;
use crate::ibc_application::PacketReceiveFailing;
use crate::response::AppResponseExt;
use crate::{
    chain_helper::ChainHelper, error::AppResult, ibc::IbcChannelWrapper,
    ibc_application::PacketReceiveOk, ibc_applications::ics20::ICS20DB, router::RouterWrapper,
};

use super::ics20::FungibleTokenPacketAck;
use super::middleware::{
    IbcAndStargate, Middleware, MiddlewareResponse, MiddlewareUniqueResponse, PacketToNext,
};

pub struct IbcHook {
    pub inner: Box<dyn IbcAndStargate>,
}

impl IbcHook {
    pub fn new<T: IbcAndStargate + 'static>(inner: T) -> Self {
        Self {
            inner: Box::new(inner),
        }
    }
}

impl Middleware for IbcHook {
    fn get_inner(&self) -> &dyn IbcAndStargate {
        &*self.inner
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
        _api: &dyn Api,
        _block: &BlockInfo,
        _router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        packet: IbcPacketReceiveMsg,
    ) -> InfallibleResult<MiddlewareResponse<PacketReceiveOk, PacketToNext>, PacketReceiveFailing>
    {
        let clos = || -> AppResult<MiddlewareResponse<PacketReceiveOk, PacketToNext>> {
            let mut data: FungibleTokenPacketData = from_json(&packet.packet.data)?;

            if data.memo != *"" {
                serde_json::from_str::<MemoField<Value>>(&data.memo)?;

                let chain_helper = ChainHelper::load(*storage.borrow())?;

                // Create ibc_hook_sender address;
                data.receiver = IbcHookHelper::parse_ibc_hooker_sender(
                    &chain_helper.chain_prefix,
                    &data.sender,
                    &packet.packet.dest.channel_id,
                )?;

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
            Ok(response) => InfallibleResult::Ok(response),
            Err(..) => InfallibleResult::Ok(MiddlewareResponse::Continue(PacketToNext { packet })),
        }
    }

    fn mid_packet_receive_after(
        &self,
        _api: &dyn Api,
        _block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        original_packet: IbcPacketReceiveMsg,
        forwarded_packet: IbcPacketReceiveMsg,
        forwarded_response: PacketReceiveOk,
    ) -> InfallibleResult<PacketReceiveOk, PacketReceiveFailing> {
        let clos = || -> AppResult<InfallibleResult<PacketReceiveOk, PacketReceiveFailing>> {
            if original_packet != forwarded_packet {
                let data: FungibleTokenPacketData = from_json(&original_packet.packet.data)?;

                let wasm_field = serde_json::from_str::<MemoField<Value>>(&data.memo)?;

                let mut ics20_db = ICS20DB.load(*storage.borrow())?;

                let (denom, _) = ics20_db.denom_from_packet(&original_packet)?;

                let chain_helper = ChainHelper::load(*storage.borrow())?;

                if let Some(wasm) = wasm_field.wasm {
                    match router.execute(
                        Addr::unchecked(IbcHookHelper::parse_ibc_hooker_sender(
                            &chain_helper.chain_prefix,
                            &data.sender,
                            &original_packet.packet.dest.channel_id,
                        )?),
                        CosmosMsg::<Empty>::Wasm(WasmMsg::Execute {
                            contract_addr: wasm.contract,
                            msg: to_json_binary(&wasm.msg)?,
                            funds: vec![Coin::new(Uint128::from_str(&data.amount)?.u128(), denom)],
                        }),
                    ) {
                        Ok(response) => Ok(InfallibleResult::Ok(PacketReceiveOk {
                            response: forwarded_response.response.clone().merge(response),
                            ack: forwarded_response.ack.clone(),
                        })),
                        Err(err) => Ok(InfallibleResult::Err(PacketReceiveFailing {
                            error: err.to_string(),
                            ack: Some(
                                to_json_binary(&FungibleTokenPacketAck::Err(err.to_string()))
                                    .unwrap(),
                            ),
                        })),
                    }
                } else {
                    bail!("No wasm field found in memo")
                }
            } else {
                bail!("Packet are equals")
            }
        };

        clos().unwrap_or(InfallibleResult::Ok(forwarded_response))
    }

    fn mid_packet_ack(
        &self,
        _api: &dyn Api,
        _block: &BlockInfo,
        _router: &RouterWrapper,
        _storage: Rc<RefCell<&mut dyn Storage>>,
        _msg: IbcPacketAckMsg,
    ) -> AppResult<MiddlewareUniqueResponse<AppResponse>> {
        Ok(MiddlewareResponse::Continue(AppResponse::default()))
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

struct IbcHookHelper;

impl IbcHookHelper {
    fn parse_ibc_hooker_sender(
        local_chain_prefix: &str,
        remote_add: &str,
        channel: &str,
    ) -> AppResult<String> {
        let sender_prefix = "ibc-wasm-hook-intermediary";
        let mut sha = Sha256::new();
        sha.update(sender_prefix.as_bytes());
        let th = sha.finalize_reset();
        sha.update(th);
        sha.update(format!("{}/{}", channel, remote_add).as_bytes());

        Ok(bech32_encode::<Bech32>(
            Hrp::parse(local_chain_prefix)?,
            sha.clone().finalize().as_slice(),
        )?)
    }
}

#[derive(Serialize, Deserialize)]
pub struct MemoField<T: Serialize> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm: Option<WasmField<T>>,
}

#[derive(Serialize, Deserialize)]
pub struct WasmField<T: Serialize> {
    pub contract: String,
    pub msg: T,
}

#[test]
fn test() {
    let result = IbcHookHelper::parse_ibc_hooker_sender(
        "osmo",
        "juno12smx2wdlyttvyzvzg54y2vnqwq2qjatezqwqxu",
        "channel-0",
    )
    .unwrap();

    assert_eq!(
        result,
        "osmo1nt0pudh879m6enw4j6z4mvyu3vmwawjv5gr7xw6lvhdsdpn3m0qs74xdjl"
    )
}
