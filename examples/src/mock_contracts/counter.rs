use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    entry_point, from_json, to_json_binary, Binary, Deps, DepsMut, Env, Ibc3ChannelOpenResponse,
    IbcBasicResponse, IbcChannelCloseMsg, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcMsg,
    IbcPacketAckMsg, IbcPacketReceiveMsg, IbcPacketTimeoutMsg, IbcReceiveResponse, MessageInfo,
    Never, Reply, Response, StdError, StdResult,
};
use cw_iper_test::ibc_applications::IBCLifecycleComplete;
use cw_storage_plus::Item;
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
pub enum CounterQueryMsg {
    Config,
}

#[cw_serde]
pub enum SudoMsg {
    #[serde(rename = "ibc_lifecycle_complete")]
    IBCLifecycleComplete(IBCLifecycleComplete),
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub enum CounterPacketData {
    Ok,
    Fail,
}

#[cw_serde]
pub enum CounterAckData {
    Ok,
    Fail,
}

#[derive(Default)]
#[cw_serde]
pub struct CounterConfig {
    pub counter_ibc_callback: u64,
    pub counter_packet_receive: u64,
    pub counter_packet_ack_ok: u64,
    pub counter_packet_ack_failing: u64,
    pub counter_ibc_hook: u64,
}

pub const COUNTER_CONFIG: Item<CounterConfig> = Item::new("counter_config");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    COUNTER_CONFIG.save(deps.storage, &CounterConfig::default())?;
    Ok(Response::new().add_attribute("action", "init"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::SendPacket(msg) => Ok(Response::new().add_message(msg)),
        ExecuteMsg::JustReceive { msg, to_fail } => {
            COUNTER_CONFIG.update(deps.storage, |mut val| -> StdResult<_> {
                val.counter_ibc_hook += 1;
                Ok(val)
            })?;

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
pub fn query(deps: Deps, _env: Env, msg: CounterQueryMsg) -> StdResult<Binary> {
    match msg {
        CounterQueryMsg::Config => to_json_binary(&COUNTER_CONFIG.load(deps.storage)?),
    }
}

#[entry_point]
pub fn _reply(_deps: DepsMut, _env: Env, _reply: Reply) -> Result<Response, ContractError> {
    unimplemented!();
}

#[entry_point]
pub fn sudo(deps: DepsMut, _env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    match msg {
        SudoMsg::IBCLifecycleComplete(ibc_callback) => {
            COUNTER_CONFIG.update(deps.storage, |mut val| -> StdResult<_> {
                val.counter_ibc_callback += 1;
                Ok(val)
            })?;
            // Only for testing purposes, if ack just return default response.
            // On timeout, raise an error.
            // This should rever the state of the contract but not the whole transaction,
            // Allowing to the ics20 module to revert the ics20 transfer.
            match ibc_callback {
                IBCLifecycleComplete::IBCAck { .. } => {
                    Ok(Response::new().add_attribute("action", "ack"))
                }
                IBCLifecycleComplete::IBCTimeout { .. } => Err(ContractError::Std(
                    StdError::generic_err("IBCTimeout detected on sudo"),
                )),
            }
        }
    }
}

#[entry_point]
pub fn _migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    unimplemented!();
}

#[entry_point]
pub fn ibc_channel_open(
    _deps: DepsMut,
    _env: Env,
    _msg: IbcChannelOpenMsg,
) -> Result<Option<Ibc3ChannelOpenResponse>, ContractError> {
    Ok(None)
}

#[entry_point]
pub fn ibc_channel_close(
    _deps: DepsMut,
    _env: Env,
    _channel: IbcChannelCloseMsg,
) -> Result<IbcBasicResponse, ContractError> {
    unimplemented!();
}

#[entry_point]
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

pub fn ibc_packet_receive(
    deps: DepsMut,
    _env: Env,
    msg: IbcPacketReceiveMsg,
) -> Result<IbcReceiveResponse, Never> {
    println!("\nPacket_received: {:#?}", msg);
    let ack = || -> StdResult<_> {
        match from_json::<CounterPacketData>(&msg.packet.data)? {
            CounterPacketData::Ok => {
                COUNTER_CONFIG.update(deps.storage, |mut val| -> StdResult<_> {
                    val.counter_packet_receive += 1;
                    Ok(val)
                })?;
                Ok(CounterAckData::Ok)
            }
            CounterPacketData::Fail => Ok(CounterAckData::Fail),
        }
    }()
    .unwrap_or(CounterAckData::Fail);

    Ok(IbcReceiveResponse::new(to_json_binary(&ack).unwrap()))
}

#[entry_point]
pub fn ibc_packet_ack(
    deps: DepsMut,
    _env: Env,
    msg: IbcPacketAckMsg,
) -> Result<IbcBasicResponse, ContractError> {
    println!("\nPacket_ack: {:#?}", msg);

    if let Ok(a) = from_json::<CounterAckData>(msg.acknowledgement.data) {
        if let CounterAckData::Ok = a {
            COUNTER_CONFIG.update(deps.storage, |mut val| -> StdResult<_> {
                val.counter_packet_ack_ok += 1;
                Ok(val)
            })?;
            return Ok(IbcBasicResponse::default());
        }
    };

    COUNTER_CONFIG.update(deps.storage, |mut val| -> StdResult<_> {
        val.counter_packet_ack_failing += 1;
        Ok(val)
    })?;

    Ok(IbcBasicResponse::default())
}

#[entry_point]
pub fn ibc_packet_timeout(
    _deps: DepsMut,
    _env: Env,
    _msg: IbcPacketTimeoutMsg,
) -> Result<IbcBasicResponse, ContractError> {
    // println!("Packet_timeout: {:?}", msg);
    Ok(IbcBasicResponse::default())
}
