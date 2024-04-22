use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use anyhow::{anyhow, bail};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    to_json_binary, Addr, IbcChannel, IbcEndpoint, IbcMsg, IbcOrder, IbcTimeout, IbcTimeoutBlock,
    Timestamp,
};
use ibc_proto::ibc::{apps::transfer::v2::FungibleTokenPacketData, core::client::v1::Height};

use crate::{
    error::AppResult,
    ibc_application::IbcApplication,
    ibc_module::{IbcPacketType, OutgoingPacket},
};

#[derive(Clone)]
#[non_exhaustive]
pub struct IbcChannelWrapper {
    pub local: IbcChannelCreator,
    pub remote: IbcChannelCreator,
    pub status: IbcChannelStatus,
    pub sequence: Rc<RefCell<u64>>,
}

impl IbcChannelWrapper {
    pub fn new(
        local: IbcChannelCreator,
        remote: IbcChannelCreator,
        sequence: Rc<RefCell<u64>>,
    ) -> Self {
        Self {
            local,
            remote,
            status: IbcChannelStatus::Created,
            sequence,
        }
    }
}

#[derive(Default, Clone)]
pub struct Channels {
    channels: BTreeMap<u64, IbcChannelWrapper>,
}

impl Channels {
    pub fn get(&self, id: impl Channelable) -> AppResult<&IbcChannelWrapper> {
        self.channels
            .get(&id.as_channel_number()?)
            .ok_or(anyhow!("channel not found"))
    }

    pub fn get_mut(&mut self, id: impl Channelable) -> AppResult<&mut IbcChannelWrapper> {
        self.channels
            .get_mut(&id.as_channel_number()?)
            .ok_or(anyhow!("channel not found"))
    }

    pub fn next_key(&self) -> u64 {
        self.channels
            .last_key_value()
            .map(|(k, _)| k + 1)
            .unwrap_or(0)
    }

    pub fn insert(&mut self, key: impl Channelable, channel: IbcChannelWrapper) -> AppResult<()> {
        let key = key.as_channel_number()?;
        self.channels.insert(key, channel);
        Ok(())
    }
}

pub trait Channelable {
    fn as_channel_string(&self) -> String;
    fn as_channel_number(&self) -> AppResult<u64>;
}

impl Channelable for u64 {
    fn as_channel_string(&self) -> String {
        format!("channel-{}", self)
    }

    fn as_channel_number(&self) -> AppResult<u64> {
        Ok(self.clone())
    }
}

impl Channelable for String {
    fn as_channel_string(&self) -> String {
        self.clone()
    }

    fn as_channel_number(&self) -> AppResult<u64> {
        self.strip_prefix("channel-")
            .ok_or(anyhow!("invalid channel id"))
            .and_then(|s| s.parse::<u64>().map_err(|_| anyhow!("invalid channel id")))
    }
}

impl Channelable for &str {
    fn as_channel_string(&self) -> String {
        self.to_string()
    }

    fn as_channel_number(&self) -> AppResult<u64> {
        self.strip_prefix("channel-")
            .ok_or(anyhow!("invalid channel id"))
            .and_then(|s| s.parse::<u64>().map_err(|_| anyhow!("invalid channel id")))
    }
}

#[cw_serde]
pub enum IbcChannelStatus {
    Created,
    Opening,
    Connected,
    Closed,
}

impl IbcChannelStatus {
    pub fn to_next_status(&mut self) -> AppResult<()> {
        match self {
            IbcChannelStatus::Created => *self = IbcChannelStatus::Opening,
            IbcChannelStatus::Opening => *self = IbcChannelStatus::Connected,
            _ => bail!("invalid status for next: {:?}", self),
        }

        Ok(())
    }
}

#[cw_serde]
pub enum IbcPort {
    Contract(Addr),
    Module(String),
}

impl IbcPort {
    pub fn port_name(&self) -> String {
        match self {
            IbcPort::Contract(addr) => addr.to_string(),
            IbcPort::Module(name) => name.clone(),
        }
    }

    pub fn from_application(ibc_application: impl IbcApplication) -> Self {
        Self::Module(ibc_application.port_name())
    }
}

#[cw_serde]
#[non_exhaustive]
pub struct IbcChannelCreator {
    pub port: IbcPort,
    pub order: IbcOrder,
    pub version: String,
    pub connection_id: String,
    pub chain_id: String,
    channel_id: Option<u64>,
}

impl IbcChannelCreator {
    pub fn new(
        port: IbcPort,
        order: IbcOrder,
        version: impl Into<String>,
        connection_id: impl Into<String>,
        chain_id: impl Into<String>,
    ) -> Self {
        Self {
            port,
            order,
            version: version.into(),
            connection_id: connection_id.into(),
            chain_id: chain_id.into(),
            channel_id: None,
        }
    }

    pub fn channel_id(&self) -> AppResult<u64> {
        self.channel_id.ok_or(anyhow!("channel-id not set"))
    }

    pub fn set_channel_id(&mut self, channe_id: u64) {
        self.channel_id = Some(channe_id);
    }

    pub fn as_endpoint(&self) -> AppResult<IbcEndpoint> {
        Ok(IbcEndpoint {
            port_id: self.port.port_name(),
            channel_id: self
                .channel_id
                .ok_or(anyhow!("channel-id not set"))?
                .as_channel_string(),
        })
    }
}

pub trait IbcChannelExt {
    fn new_from_creators(
        local: &IbcChannelCreator,
        remote: &IbcChannelCreator,
    ) -> AppResult<IbcChannel>;
}

impl IbcChannelExt for IbcChannel {
    fn new_from_creators(
        local: &IbcChannelCreator,
        remote: &IbcChannelCreator,
    ) -> AppResult<IbcChannel> {
        Ok(IbcChannel::new(
            local.as_endpoint()?,
            remote.as_endpoint()?,
            local.order.clone(),
            local.version.clone(),
            local.connection_id.clone(),
        ))
    }
}

pub trait IbcMsgExt {
    fn get_src_channel(&self) -> String;
    fn into_packet(
        self,
        sender: &Addr,
        channel_wrapper: &IbcChannelWrapper,
    ) -> AppResult<IbcPacketType>;
}

impl IbcMsgExt for IbcMsg {
    fn get_src_channel(&self) -> String {
        match self {
            IbcMsg::Transfer { channel_id, .. } => channel_id.clone(),
            IbcMsg::SendPacket { channel_id, .. } => channel_id.clone(),
            IbcMsg::CloseChannel { channel_id } => channel_id.clone(),
            _ => todo!(),
        }
    }

    fn into_packet(
        self,
        sender: &Addr,
        channel_wrapper: &IbcChannelWrapper,
    ) -> AppResult<IbcPacketType> {
        let src = channel_wrapper.local.as_endpoint()?;
        let dest = channel_wrapper.remote.as_endpoint()?;

        match self {
            IbcMsg::Transfer {
                to_address,
                amount,
                timeout,
                memo,
                ..
            } => Ok(IbcPacketType::OutgoingPacket(OutgoingPacket {
                data: to_json_binary(&FungibleTokenPacketData {
                    denom: amount.denom,
                    amount: amount.amount.to_string(),
                    sender: sender.to_string(),
                    receiver: to_address,
                    memo: memo.unwrap_or_default(),
                })?,
                src,
                dest,
                timeout,
            })),
            IbcMsg::SendPacket { data, timeout, .. } => {
                Ok(IbcPacketType::OutgoingPacket(OutgoingPacket {
                    data,
                    src,
                    dest,
                    timeout,
                }))
            }
            IbcMsg::CloseChannel { channel_id } => Ok(IbcPacketType::CloseChannel { channel_id }),
            _ => unimplemented!(),
        }
    }
}

pub fn create_ibc_timeout(nanos: u64, height: Option<Height>) -> IbcTimeout {
    match (nanos, height) {
        (0, None) => unimplemented!(),
        (0, Some(height)) => {
            return IbcTimeout::with_block(IbcTimeoutBlock {
                revision: height.revision_number,
                height: height.revision_height,
            });
        }
        (seconds, None) => {
            return IbcTimeout::with_timestamp(Timestamp::from_nanos(seconds));
        }

        (seconds, Some(height)) => {
            return IbcTimeout::with_both(
                IbcTimeoutBlock {
                    revision: height.revision_number,
                    height: height.revision_height,
                },
                Timestamp::from_nanos(seconds),
            );
        }
    }
}
