use anyhow::bail;

use bech32::{encode as bech32_encode, Bech32, Hrp};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Coin, Empty, Uint128};
use serde::{Deserialize, Serialize};

use std::str::FromStr;
use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{
    from_json, to_json_binary, Addr, Api, BlockInfo, CosmosMsg, IbcPacket, IbcPacketReceiveMsg,
    Storage, WasmMsg,
};
use cw_multi_test::{AppResponse, SudoMsg, WasmSudo};
use ibc_proto::ibc::apps::transfer::v2::FungibleTokenPacketData;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::ibc_application::PacketReceiveFailing;
use crate::ibc_module::{AckPacket, TimeoutPacket};
use crate::iper_app::InfallibleResult;
use crate::middleware::{IbcAndStargate, MidRecFailing, MidRecOk, Middleware, MiddlewareResponse};
use crate::{
    chain_helper::ChainHelper, error::AppResult, ibc_application::PacketReceiveOk,
    ibc_applications::ics20::ICS20DB, router::RouterWrapper,
};

use super::ics20::FungibleTokenPacketAck;
use super::MemoField;

/// `IbcHook` implementation as [`Middleware`]
pub struct IbcHook {
    /// Inner [`IbcApplication`](crate::ibc_application::IbcApplication). It should be [`Ics20`](crate::ibc_applications::Ics20)
    /// or another [`Middleware`] that wrap [`Ics20`](crate::ibc_applications::Ics20).
    pub inner: Box<dyn IbcAndStargate>,
}

impl IbcHook {
    /// Constructor
    pub fn new<T: IbcAndStargate + 'static>(inner: T) -> Self {
        Self {
            inner: Box::new(inner),
        }
    }

    fn try_handle_callback(
        &self,
        api: &dyn Api,
        _block: &BlockInfo,
        router: &RouterWrapper,
        _storage: Rc<RefCell<&mut dyn Storage>>,
        packet: AckOrTimeout,
    ) -> AppResult<AppResponse> {
        let data: FungibleTokenPacketData = from_json(packet.get_original_packet().data)?;

        if let Ok(wasm_field) = serde_json::from_str::<MemoField<Value>>(&data.memo) {
            if let Some(contract_addr) = wasm_field.ibc_callback {
                let msg = match packet {
                    AckOrTimeout::Ack(ack) => IBCLifecycleComplete::IBCAck {
                        channel: ack.original_packet.packet.src.channel_id,
                        sequence: ack.original_packet.packet.sequence,
                        ack: ack.ack.to_base64(),
                        success: ack.success,
                    },
                    AckOrTimeout::Timeout(timeout) => IBCLifecycleComplete::IBCTimeout {
                        channel: timeout.original_packet.packet.src.channel_id,
                        sequence: timeout.original_packet.packet.sequence,
                    },
                };

                let response = router.try_sudo(SudoMsg::Wasm(WasmSudo {
                    contract_addr: api.addr_validate(&contract_addr)?,
                    message: to_json_binary(&IbcHookSudoMsg::IBCLifecycleComplete(msg))?,
                }));

                println!("Response: {:?}", response);
            }
        }

        Ok(AppResponse::default())
    }
}

impl Middleware for IbcHook {
    fn get_inner(&self) -> &dyn IbcAndStargate {
        &*self.inner
    }

    fn mid_packet_receive_before(
        &self,
        _api: &dyn Api,
        _block: &BlockInfo,
        _router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        packet: IbcPacketReceiveMsg,
    ) -> InfallibleResult<
        MiddlewareResponse<PacketReceiveOk, IbcPacketReceiveMsg>,
        PacketReceiveFailing,
    > {
        let clos = || -> AppResult<MiddlewareResponse<PacketReceiveOk, IbcPacketReceiveMsg>> {
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

                Ok(MiddlewareResponse::Continue(forwarded_packet))
            } else {
                Ok(MiddlewareResponse::Continue(packet.clone()))
            }
        };

        match clos() {
            Ok(response) => InfallibleResult::Ok(response),
            Err(..) => InfallibleResult::Ok(MiddlewareResponse::Continue(packet)),
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
        forwarded_response: InfallibleResult<PacketReceiveOk, PacketReceiveFailing>,
    ) -> InfallibleResult<MidRecOk, MidRecFailing> {
        let clos = || -> AppResult<InfallibleResult<MidRecOk, MidRecFailing>> {
            if original_packet != forwarded_packet && forwarded_response.is_ok() {
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
                        Ok(response) => Ok(InfallibleResult::Ok(MidRecOk::use_children(response))),
                        Err(err) => Ok(InfallibleResult::Err(MidRecFailing::new(
                            err.to_string(),
                            to_json_binary(&FungibleTokenPacketAck::Err(err.to_string()))?,
                        ))),
                    }
                } else {
                    bail!("No wasm field found in memo")
                }
            } else {
                bail!("Skip")
            }
        };

        clos().unwrap_or(InfallibleResult::Ok(MidRecOk::default()))
    }

    fn mid_packet_ack_after(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        original_packet: AckPacket,
        _forwarded_packet: AckPacket,
        _returning_reponse: AppResponse,
    ) -> AppResult<AppResponse> {
        self.try_handle_callback(
            api,
            block,
            router,
            storage,
            AckOrTimeout::Ack(original_packet),
        )
    }

    fn mid_packet_timeout_after(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        original_packet: TimeoutPacket,
        _forwarded_packet: TimeoutPacket,
        _returning_reponse: AppResponse,
    ) -> AppResult<AppResponse> {
        self.try_handle_callback(
            api,
            block,
            router,
            storage,
            AckOrTimeout::Timeout(original_packet),
        )
    }
}

/// Message type for `sudo` entry_point triggered during `ibc_callback`
#[cw_serde]
pub enum IbcHookSudoMsg {
    /// `ibc_callback` msg variant
    #[serde(rename = "ibc_lifecycle_complete")]
    IBCLifecycleComplete(IBCLifecycleComplete),
}

///
#[cw_serde]
pub enum IBCLifecycleComplete {
    #[serde(rename = "ibc_ack")]
    /// Acknowledge from destination chain
    IBCAck {
        /// The source channel (osmosis side) of the IBC packet
        channel: String,
        /// The sequence number that the packet was sent with
        sequence: u64,
        /// String encoded version of the `Ack` as seen by OnAcknowledgementPacket(..)
        ack: String,
        /// Weather an `Ack` is a success of failure according to the transfer spec
        success: bool,
    },
    /// Timeout from destination chain
    #[serde(rename = "ibc_timeout")]
    IBCTimeout {
        /// The source channel (osmosis side) of the IBC packet
        channel: String,
        /// The sequence number that the packet was sent with
        sequence: u64,
    },
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

enum AckOrTimeout {
    Ack(AckPacket),
    Timeout(TimeoutPacket),
}

impl AckOrTimeout {
    fn get_original_packet(&self) -> IbcPacket {
        match self {
            AckOrTimeout::Ack(ack) => ack.original_packet.packet.clone(),
            AckOrTimeout::Timeout(timeout) => timeout.original_packet.packet.clone(),
        }
    }
}

/// Wasm field of [`MemoField`] to trigger `ibc hook`
#[derive(Serialize, Deserialize)]
pub struct WasmField<T: Serialize> {
    /// `contract address` to trigger
    pub contract: String,
    /// serialized `ExecuteMsg`
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
