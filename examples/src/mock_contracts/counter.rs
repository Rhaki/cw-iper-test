use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    entry_point, Binary, Deps, DepsMut, Env, Ibc3ChannelOpenResponse, IbcBasicResponse,
    IbcChannelCloseMsg, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcMsg, IbcPacketAckMsg,
    IbcPacketReceiveMsg, IbcPacketTimeoutMsg, IbcReceiveResponse, MessageInfo, Never, Reply,
    Response, StdError, StdResult,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),
}

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    SendPacket(IbcMsg),
    JustReceive { msg: String, to_fail: bool },
}

#[cw_serde]
pub enum QueryMsg {}

#[cw_serde]
pub enum SudoMsg {}

#[cw_serde]
pub struct MigrateMsg {}

#[entry_point]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    Ok(Response::new().add_attribute("action", "init"))
}

#[entry_point]
pub fn execute(
    _deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::SendPacket(msg) => Ok(Response::new().add_message(msg)),
        ExecuteMsg::JustReceive { msg, to_fail } => {
            if to_fail {
                Err(ContractError::Std(StdError::generic_err(msg)))
            } else {
                Ok(Response::new()
                    .add_attribute("sender", info.sender)
                    .add_attribute("msg", msg)
                    .add_attribute("coins", format!("{:#?}", info.funds)))
            }
        }
    }
}

#[entry_point]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    unimplemented!();
}

#[entry_point]
pub fn _reply(_deps: DepsMut, _env: Env, _reply: Reply) -> Result<Response, ContractError> {
    unimplemented!();
}

#[entry_point]
pub fn _sudo(_deps: DepsMut, _env: Env, _msg: SudoMsg) -> Result<Response, ContractError> {
    unimplemented!();
}

#[entry_point]
pub fn _migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    unimplemented!();
}

#[entry_point]
/// Enforces ordering and versioning constraints
pub fn ibc_channel_open(
    _deps: DepsMut,
    _env: Env,
    _msg: IbcChannelOpenMsg,
) -> Result<Option<Ibc3ChannelOpenResponse>, ContractError> {
    Ok(None)
}

/// Not handled yet
/// Should the contract remove the channel from the storage?
#[entry_point]
pub fn ibc_channel_close(
    _deps: DepsMut,
    _env: Env,
    _channel: IbcChannelCloseMsg,
) -> Result<IbcBasicResponse, ContractError> {
    unimplemented!();
}

#[entry_point]
/// Record the channel in CHANNEL_INFO
pub fn ibc_channel_connect(
    _deps: DepsMut,
    _env: Env,
    msg: IbcChannelConnectMsg,
) -> Result<IbcBasicResponse, ContractError> {
    match msg {
        IbcChannelConnectMsg::OpenAck {
            channel: _,
            counterparty_version: _,
        } => println!("Connect ack"),
        IbcChannelConnectMsg::OpenConfirm { channel: _ } => println!("Connect confirm"),
    }

    Ok(IbcBasicResponse::default())
}

#[entry_point]
/// Receive a ibc packet.
/// packet.data will be deserialized into GatePacket.
/// If the packet contain `SendNativeInfo`, we store the packet and set into the ack the key for this packet (it will be executed when the contract will receives the native token).
/// Otherwise we proceed executing the `Requests` contained in the packet.
pub fn ibc_packet_receive(
    _deps: DepsMut,
    _env: Env,
    msg: IbcPacketReceiveMsg,
) -> Result<IbcReceiveResponse, Never> {
    println!("Packet_received: {:?}", msg);
    Ok(IbcReceiveResponse::new(msg.packet.data))
}

#[entry_point]
/// Matching the result in ack.data with `AckType` and execute the variant
pub fn ibc_packet_ack(
    _deps: DepsMut,
    _env: Env,
    msg: IbcPacketAckMsg,
) -> Result<IbcBasicResponse, ContractError> {
    println!("Packet_ack: {:?}", msg);
    Ok(IbcBasicResponse::default())
}

#[entry_point]
/// Same case handled on `ibc_packet_ack` in `Error` scenario
pub fn ibc_packet_timeout(
    _deps: DepsMut,
    _env: Env,
    _msg: IbcPacketTimeoutMsg,
) -> Result<IbcBasicResponse, ContractError> {
    unimplemented!();
}
