use std::{cell::RefCell, rc::Rc};

use anyhow::anyhow;
use cosmwasm_std::{
    from_json, to_json_binary, Addr, Api, BankMsg, Binary, BlockInfo, Coin, CosmosMsg, Empty,
    GrpcQuery, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcMsg, IbcPacketAckMsg,
    IbcPacketReceiveMsg, Storage, Uint128,
};
use cw_iper_test_macros::{urls_int, IbcPort, Stargate};
use cw_multi_test::{AppResponse, BankSudo, SudoMsg};

use ibc_proto::ibc::apps::transfer::v1::MsgTransfer;
use ibc_proto::ibc::apps::transfer::v2::FungibleTokenPacketData;
use serde::{Deserialize, Serialize};

use crate::ibc::create_ibc_timeout;
use crate::ibc_application::{IbcApplication, PacketReceiveResponse};
use crate::ibc_module::{emit_packet_boxed, IbcPacketType, OutgoingPacket, OutgoingPacketRaw};

use crate::{
    error::AppResult, ibc::IbcChannelWrapper, router::RouterWrapper, stargate::StargateApplication,
};
use prost::Message;

use std::str::FromStr;
#[derive(Default, Clone, IbcPort, Stargate)]
#[ibc_port = "transfer"]
#[stargate(name = "ics20", query_urls = Ics20QueryUrls, msgs_urls = Ics20MsgUrls)]
pub struct Ics20;

#[urls_int]
pub enum Ics20MsgUrls {
    #[strum(serialize = "/ibc.applications.transfer.v1.MsgTransfer")]
    MsgTransfer,
}

#[urls_int]
pub enum Ics20QueryUrls {}

impl IbcApplication for Ics20 {
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
        let (data, timeout) = match msg {
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

        let response = router.execute(
            sender,
            CosmosMsg::<Empty>::Bank(BankMsg::Burn {
                amount: vec![Coin::new(
                    Uint128::from_str(&data.amount)?,
                    data.denom.clone(),
                )],
            }),
        )?;

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
    ) -> AppResult<PacketReceiveResponse> {
        let clos = || {
            let data: FungibleTokenPacketData = from_json(&msg.packet.data)?;

            let to = api.addr_validate(&data.receiver)?;

            let coin = Coin::new(Uint128::from_str(&data.amount)?, "mock_denom".to_string());

            router.sudo(SudoMsg::Bank(BankSudo::Mint {
                to_address: to.to_string(),
                amount: vec![coin],
            }))
        };

        match clos() {
            Ok(response) => Ok(PacketReceiveResponse {
                response: AppResponse::default(),
                ack: to_json_binary(&FungibleTokenPacketAck::Ok)?,
            }),
            Err(err) => Ok(PacketReceiveResponse {
                response: AppResponse::default(),
                ack: to_json_binary(&FungibleTokenPacketAck::Err(err.to_string()))?,
            }),
        }
    }

    fn packet_ack(
        &self,
        _api: &dyn Api,
        _block: &BlockInfo,
        router: &RouterWrapper,
        _storage: Rc<RefCell<&mut dyn Storage>>,
        msg: IbcPacketAckMsg,
    ) -> AppResult<AppResponse> {
        match from_json::<FungibleTokenPacketAck>(msg.acknowledgement.data)? {
            FungibleTokenPacketAck::Ok => Ok(AppResponse::default()),
            FungibleTokenPacketAck::Err(..) => {
                let original_packet: FungibleTokenPacketData = from_json(msg.original_packet.data)?;
                router.sudo(SudoMsg::Bank(BankSudo::Mint {
                    to_address: original_packet.sender,
                    amount: vec![Coin::new(
                        Uint128::from_str(&original_packet.amount)?,
                        original_packet.denom,
                    )],
                }))
            }
        }
    }

    fn open_channel(
        &self,
        _api: &dyn Api,
        _block: &BlockInfo,
        _router: &RouterWrapper,
        _storage: Rc<RefCell<&mut dyn Storage>>,
        _msg: IbcChannelOpenMsg,
    ) -> AppResult<AppResponse> {
        println!("channel_opened");
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
        println!("channel_connect");
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
enum FungibleTokenPacketAck {
    Ok,
    Err(String),
}
