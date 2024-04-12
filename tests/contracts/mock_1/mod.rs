use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    entry_point, Binary, Deps, DepsMut, Env, Ibc3ChannelOpenResponse, IbcBasicResponse,
    IbcChannelCloseMsg, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcMsg, IbcPacketAckMsg,
    IbcPacketReceiveMsg, IbcPacketTimeoutMsg, IbcReceiveResponse, MessageInfo, Never, Reply,
    Response, StdResult,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {}

#[cw_serde]
pub struct InstantiateMsg {}

#[cw_serde]
pub enum ExecuteMsg {
    SendPacket(IbcMsg),
}

#[cw_serde]
pub enum QueryMsg {}

#[cw_serde]
pub enum SudoMsg {}

#[cw_serde]
pub struct MigrateMsg {}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    Ok(Response::new().add_attribute("action", "init"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::SendPacket(msg) => Ok(Response::new().add_message(msg)),
    }
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    unimplemented!();
}

#[entry_point]
pub fn reply(deps: DepsMut, env: Env, reply: Reply) -> Result<Response, ContractError> {
    unimplemented!();
}

#[entry_point]
pub fn sudo(deps: DepsMut, env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    unimplemented!();
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    unimplemented!();
}

#[entry_point]
/// Enforces ordering and versioning constraints
pub fn ibc_channel_open(
    _deps: DepsMut,
    _env: Env,
    msg: IbcChannelOpenMsg,
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
    deps: DepsMut,
    _env: Env,
    msg: IbcChannelConnectMsg,
) -> Result<IbcBasicResponse, ContractError> {
    match msg {
        IbcChannelConnectMsg::OpenAck {
            channel,
            counterparty_version,
        } => println!("Connect ack"),
        IbcChannelConnectMsg::OpenConfirm { channel } => println!("Connect confirm"),
    }

    Ok(IbcBasicResponse::default())
}

#[entry_point]
/// Receive a ibc packet.
/// packet.data will be deserialized into GatePacket.
/// If the packet contain `SendNativeInfo`, we store the packet and set into the ack the key for this packet (it will be executed when the contract will receives the native token).
/// Otherwise we proceed executing the `Requests` contained in the packet.
pub fn ibc_packet_receive(
    deps: DepsMut,
    env: Env,
    msg: IbcPacketReceiveMsg,
) -> Result<IbcReceiveResponse, Never> {
    println!("Packet_received: {:?}", msg);
    Ok(IbcReceiveResponse::new(msg.packet.data))
}

#[entry_point]
/// Matching the result in ack.data with `AckType` and execute the variant
pub fn ibc_packet_ack(
    deps: DepsMut,
    env: Env,
    msg: IbcPacketAckMsg,
) -> Result<IbcBasicResponse, ContractError> {
    println!("Packet_ack: {:?}", msg);
    Ok(IbcBasicResponse::default())
}

#[entry_point]
/// Same case handled on `ibc_packet_ack` in `Error` scenario
pub fn ibc_packet_timeout(
    deps: DepsMut,
    env: Env,
    msg: IbcPacketTimeoutMsg,
) -> Result<IbcBasicResponse, ContractError> {
    unimplemented!();
}
