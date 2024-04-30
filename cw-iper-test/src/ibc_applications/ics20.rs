use std::collections::BTreeMap;
use std::{cell::RefCell, rc::Rc};

use anyhow::anyhow;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    from_json, to_json_binary, Addr, Api, BankMsg, Binary, BlockInfo, Coin, CosmosMsg, Empty,
    Event, GrpcQuery, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcMsg, IbcPacketReceiveMsg,
    Storage, Uint128,
};
use cw_iper_test_macros::{urls, IbcPort, Stargate};
use cw_multi_test::{AppResponse, BankSudo, SudoMsg};

use cw_storage_plus::Item;
use ibc_proto::ibc::apps::transfer::v1::MsgTransfer;
use ibc_proto::ibc::apps::transfer::v2::FungibleTokenPacketData;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ibc::create_ibc_timeout;
use crate::ibc_app::InfallibleResult;
use crate::ibc_application::{IbcApplication, PacketReceiveFailing, PacketReceiveOk};
use crate::ibc_module::{
    emit_packet_boxed, AckPacket, IbcPacketType, OutgoingPacket, OutgoingPacketRaw, TimeoutPacket,
};

use crate::{
    error::AppResult, ibc::IbcChannelWrapper, router::RouterWrapper, stargate::StargateApplication,
};
use prost::Message;

use std::str::FromStr;
#[derive(Default, Clone, IbcPort, Stargate)]
#[ibc_port = "transfer"]
#[stargate(name = "ics20", query_urls = Ics20QueryUrls, msgs_urls = Ics20MsgUrls)]
pub struct Ics20;

#[urls]
pub enum Ics20MsgUrls {
    #[strum(serialize = "/ibc.applications.transfer.v1.MsgTransfer")]
    MsgTransfer,
}

#[urls]
pub enum Ics20QueryUrls {}

impl IbcApplication for Ics20 {
    fn init(&self, api: &cw_multi_test::MockApiBech32, storage: &mut dyn Storage) {
        let db = Ics20Db::new(api.addr_make("ics20_addr_container"));

        ICS20DB.save(storage, &db).unwrap();
    }

    fn handle_outgoing_packet(
        &self,
        _api: &dyn Api,
        _block: &BlockInfo,
        sender: Addr,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcMsg,
        channel: IbcChannelWrapper,
    ) -> AppResult<AppResponse> {
        let (mut data, timeout) = match msg {
            IbcMsg::Transfer {
                to_address,
                amount,
                timeout,
                memo,
                ..
            } => (
                FungibleTokenPacketData {
                    denom: amount.denom,
                    amount: amount.amount.to_string(),
                    sender: sender.to_string(),
                    receiver: to_address,
                    memo: memo.unwrap_or_default(),
                },
                timeout,
            ),
            IbcMsg::SendPacket { data, timeout, .. } => {
                (from_json::<FungibleTokenPacketData>(&data)?, timeout)
            }
            IbcMsg::CloseChannel { .. } => {
                unimplemented!("Close Channel not implemented yet on ICS20")
            }
            _ => todo!(),
        };

        let db = ICS20DB.load(*storage.borrow())?;

        let (packet_denom, is_local) = db.handle_outgoing(&data.denom);

        let response = if is_local {
            router.execute(
                sender,
                CosmosMsg::<Empty>::Bank(BankMsg::Send {
                    to_address: db.address_container.to_string(),
                    amount: vec![Coin::new(
                        Uint128::from_str(&data.amount)?,
                        data.denom.clone(),
                    )],
                }),
            )?
        } else {
            router.execute(
                sender,
                CosmosMsg::<Empty>::Bank(BankMsg::Burn {
                    amount: vec![Coin::new(
                        Uint128::from_str(&data.amount)?,
                        data.denom.clone(),
                    )],
                }),
            )?
        };

        data.denom = packet_denom;

        emit_packet_boxed(
            IbcPacketType::OutgoingPacket(OutgoingPacket {
                timeout,
                data: to_json_binary(&data)?,
                src: channel.local.as_endpoint()?,
                dest: channel.remote.as_endpoint()?,
            }),
            &storage,
        )?;

        Ok(response)
    }

    fn packet_receive(
        &self,
        api: &dyn Api,
        _block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcPacketReceiveMsg,
    ) -> InfallibleResult<PacketReceiveOk, PacketReceiveFailing> {
        let clos = || {
            let data: FungibleTokenPacketData = from_json(&msg.packet.data)?;

            let mut db = ICS20DB.load(*storage.borrow())?;

            let (denom, is_local) = db.handle_incoming(msg, *storage.borrow_mut())?;

            let to = api.addr_validate(&data.receiver)?;

            let coin = Coin::new(Uint128::from_str(&data.amount)?, denom);

            // Escrow the funds
            if is_local {
                router.execute(
                    db.address_container.clone(),
                    CosmosMsg::<Empty>::Bank(BankMsg::Send {
                        to_address: to.to_string(),
                        amount: vec![coin.clone()],
                    }),
                )
            // Mint new token
            } else {
                router.sudo(SudoMsg::Bank(BankSudo::Mint {
                    to_address: to.to_string(),
                    amount: vec![coin.clone()],
                }))?;
                Ok(AppResponse {
                    events: vec![Event::new("ics20_mint")
                        .add_attribute("sender", data.sender)
                        .add_attribute("receiver", to.to_string())
                        .add_attribute("amount", coin.amount)
                        .add_attribute("denom", coin.denom)],
                    data: None,
                })
            }
        };

        match clos() {
            Ok(response) => InfallibleResult::Ok(PacketReceiveOk {
                response,
                ack: Some(to_json_binary(&FungibleTokenPacketAck::Ok).unwrap()),
            }),
            Err(err) => InfallibleResult::Err(PacketReceiveFailing {
                error: err.to_string(),
                ack: Some(to_json_binary(&FungibleTokenPacketAck::Err(err.to_string())).unwrap()),
            }),
        }
    }

    fn packet_ack(
        &self,
        _api: &dyn Api,
        _block: &BlockInfo,
        router: &RouterWrapper,
        _storage: Rc<RefCell<&mut dyn Storage>>,
        msg: AckPacket,
    ) -> AppResult<AppResponse> {
        match from_json::<FungibleTokenPacketAck>(msg.ack)? {
            FungibleTokenPacketAck::Ok => Ok(AppResponse::default()),
            FungibleTokenPacketAck::Err(..) => {
                let original_packet: FungibleTokenPacketData =
                    from_json(msg.original_packet.packet.data)?;
                router.sudo(SudoMsg::Bank(BankSudo::Mint {
                    to_address: original_packet.sender.clone(),
                    amount: vec![Coin::new(
                        Uint128::from_str(&original_packet.amount)?,
                        original_packet.denom.clone(),
                    )],
                }))?;
                Ok(AppResponse {
                    events: vec![Event::new("revert_ibc_transfer")
                        .add_attribute("sender", original_packet.sender)
                        .add_attribute("amount", original_packet.amount)
                        .add_attribute("denom", original_packet.denom)],
                    data: None,
                })
            }
        }
    }

    /// Just raise Ack with mock acknowledgement
    fn packet_timeout(
        &self,
        api: &dyn Api,
        block: &BlockInfo,
        router: &RouterWrapper,
        storage: Rc<RefCell<&mut dyn Storage>>,
        msg: TimeoutPacket,
    ) -> AppResult<AppResponse> {
        let msg = AckPacket {
            ack: to_json_binary(&FungibleTokenPacketAck::Err("Timeout".to_string()))?,
            original_packet: msg.original_packet,
            success: false,
            relayer: msg.relayer,
        };

        self.packet_ack(api, block, router, storage, msg)
    }

    fn open_channel(
        &self,
        _api: &dyn Api,
        _block: &BlockInfo,
        _router: &RouterWrapper,
        _storage: Rc<RefCell<&mut dyn Storage>>,
        _msg: IbcChannelOpenMsg,
    ) -> AppResult<AppResponse> {
        Ok(AppResponse::default())
    }

    fn channel_connect(
        &self,
        _api: &dyn Api,
        _block: &BlockInfo,
        _router: &RouterWrapper,
        _storage: Rc<RefCell<&mut dyn Storage>>,
        _msg: IbcChannelConnectMsg,
    ) -> AppResult<AppResponse> {
        Ok(AppResponse::default())
    }
}

impl StargateApplication for Ics20 {
    fn stargate_msg(
        &self,
        _api: &dyn Api,
        storage: Rc<RefCell<&mut dyn Storage>>,
        router: &RouterWrapper,
        _block: &BlockInfo,
        sender: Addr,
        type_url: String,
        data: Binary,
    ) -> AppResult<AppResponse> {
        match Ics20MsgUrls::from_str(&type_url)? {
            Ics20MsgUrls::MsgTransfer => {
                let msg = MsgTransfer::decode(data.as_slice())?;

                let coin = msg.token.ok_or(anyhow!("missing token"))?;

                let packet = FungibleTokenPacketData {
                    denom: coin.denom.clone(),
                    amount: coin.amount.clone(),
                    sender: sender.to_string(),
                    receiver: msg.receiver,
                    memo: msg.memo,
                };

                let response = router.execute(
                    sender,
                    CosmosMsg::<Empty>::Bank(BankMsg::Burn {
                        amount: vec![Coin::new(
                            Uint128::from_str(&coin.amount)?,
                            coin.denom.clone(),
                        )],
                    }),
                )?;

                emit_packet_boxed(
                    IbcPacketType::OutgoinPacketRaw(OutgoingPacketRaw {
                        data: to_json_binary(&packet)?,
                        src_port: msg.source_port,
                        src_channel: msg.source_channel,
                        timeout: create_ibc_timeout(msg.timeout_timestamp, msg.timeout_height),
                    }),
                    &storage,
                )?;

                Ok(response)
            }
        }
    }

    fn stargate_query(
        &self,
        _api: &dyn Api,
        _storage: &dyn Storage,
        _querier: &dyn cosmwasm_std::Querier,
        _block: &BlockInfo,
        _request: GrpcQuery,
    ) -> AppResult<cosmwasm_std::Binary> {
        todo!()
    }
}

#[derive(Serialize, Deserialize)]
pub enum FungibleTokenPacketAck {
    Ok,
    Err(String),
}

pub const ICS20DB: Item<Ics20Db> = Item::new("ics20_db");

#[cw_serde]
pub struct Ics20Db {
    pub incoming_denoms: BTreeMap<IbcDenom, Trace>,
    pub address_container: Addr,
}

impl Ics20Db {
    pub fn new(address_container: Addr) -> Self {
        Self {
            address_container,
            incoming_denoms: BTreeMap::new(),
        }
    }

    pub fn handle_outgoing(&self, denom: &str) -> (String, bool) {
        if denom.starts_with("ibc/") {
            let trace = self.incoming_denoms.get(denom).unwrap();
            (trace.clone(), false)
        } else {
            (denom.to_string(), true)
        }
    }

    pub fn handle_incoming(
        &mut self,
        msg: IbcPacketReceiveMsg,
        storage: &mut dyn Storage,
    ) -> AppResult<(String, bool)> {
        let (denom, is_local) = self.denom_from_packet(&msg)?;

        ICS20DB.save(storage, self)?;

        Ok((denom, is_local))
    }

    pub fn denom_from_packet(&mut self, msg: &IbcPacketReceiveMsg) -> AppResult<(String, bool)> {
        let data: FungibleTokenPacketData = from_json(&msg.packet.data)?;

        let src_trace = format!("{}/{}/", msg.packet.src.port_id, msg.packet.src.channel_id);

        match data.denom.strip_prefix(&src_trace) {
            // Has been sent from this chain
            Some(original_denom) => {
                let denom = if original_denom.starts_with("transfer/") {
                    Ics20Helper::compute_ibc_denom_from_trace(original_denom)
                } else {
                    original_denom.to_string()
                };

                Ok((denom, true))
            }
            // Not sent from this chain
            None => {
                let new_trace = format!(
                    "{}/{}/{}",
                    msg.packet.dest.port_id, msg.packet.dest.channel_id, data.denom
                );
                let denom = Ics20Helper::compute_ibc_denom_from_trace(&new_trace);
                self.incoming_denoms.insert(denom.clone(), new_trace);

                Ok((denom, false))
            }
        }
    }
}

type IbcDenom = String;

type Trace = String;

pub struct Ics20Helper;

impl Ics20Helper {
    pub fn compute_ibc_denom_from_trace(trace: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(trace);
        format!("ibc/{}", format!("{:x}", hasher.finalize()).to_uppercase())
    }
}

#[test]
#[rustfmt::skip]
fn test_path() {
    let path = "transfer/channel-6";
    let base_denom = "uusdc";
    let denom = Ics20Helper::compute_ibc_denom_from_trace(&format!("{}/{}",path, base_denom));
    assert_eq!(denom, "ibc/B3504E092456BA618CC28AC671A71FB08C6CA0FD0BE7C8A5B5A3E2DD933CC9E4");

    let path = "transfer/channel-0/transfer/channel-141/transfer/channel-42/transfer/channel-27";
    let base_denom = "uluna";
    let denom = Ics20Helper::compute_ibc_denom_from_trace(&format!("{}/{}",path, base_denom));

    println!("{}", denom);
}

#[derive(Serialize, Deserialize)]
pub struct MemoField<T: Serialize> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm: Option<WasmField<T>>,
    pub ibc_callback: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct WasmField<T: Serialize> {
    pub contract: String,
    pub msg: T,
}
