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
    ibc_module::{IbcPacketType, OutgoingPacket},
    IbcApplication,
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
        Ok(*self)
    }
}

impl Channelable for String {
    fn as_channel_string(&self) -> String {
        self.clone()
    }

    fn as_channel_number(&self) -> AppResult<u64> {
        self.strip_prefix("channel-")
            .ok_or(anyhow!("invalid `channel-id`"))
            .and_then(|s| {
                s.parse::<u64>()
                    .map_err(|_| anyhow!("invalid `channel-id`"))
            })
    }
}

impl Channelable for &str {
    fn as_channel_string(&self) -> String {
        self.to_string()
    }

    fn as_channel_number(&self) -> AppResult<u64> {
        self.strip_prefix("channel-")
            .ok_or(anyhow!("invalid `channel-id`"))
            .and_then(|s| {
                s.parse::<u64>()
                    .map_err(|_| anyhow!("invalid `channel-id`"))
            })
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
    #[allow(clippy::wrong_self_convention)]
    pub fn to_next_status(&mut self) -> AppResult<()> {
        match self {
            IbcChannelStatus::Created => *self = IbcChannelStatus::Opening,
            IbcChannelStatus::Opening => *self = IbcChannelStatus::Connected,
            _ => bail!("invalid status for next: {:?}", self),
        }

        Ok(())
    }
}

/// Define the `port` type of a ibc-channel
#[cw_serde]
pub enum IbcPort {
    /// `smart-contract` port address. The contract has to implement the `ibc entry_points`.
    Contract(Addr),
    /// [`IbcApplication`](crate::ibc_application::IbcApplication) port name.
    Module(String),
}

impl IbcPort {
    pub(crate) fn port_name(&self) -> String {
        match self {
            IbcPort::Contract(addr) => addr.to_string(),
            IbcPort::Module(name) => name.clone(),
        }
    }

    /// Create a a [`IbcPort`] from [`IbcApplication`]
    pub fn from_application(ibc_application: impl IbcApplication) -> Self {
        Self::Module(ibc_application.port_name())
    }
}

///
#[cw_serde]
#[non_exhaustive]
pub struct IbcChannelCreator {
    /// Channel `port`
    pub port: IbcPort,
    /// Channel packet `order`
    pub order: IbcOrder,
    /// Channel packet `version`
    pub version: String,
    /// Channel `connection_id`
    pub connection_id: String,
    /// Chain name. This value has to be equal to [`IperApp::chain_id`](crate::iper_app::IperApp)
    pub chain_id: String,
    channel_id: Option<u64>,
}

impl IbcChannelCreator {
    /// Constructor function
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

    pub(crate) fn channel_id(&self) -> AppResult<u64> {
        self.channel_id.ok_or(anyhow!("channel-id not set"))
    }

    pub(crate) fn set_channel_id(&mut self, channe_id: u64) {
        self.channel_id = Some(channe_id);
    }

    pub(crate) fn as_endpoint(&self) -> AppResult<IbcEndpoint> {
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
        (0, Some(height)) => IbcTimeout::with_block(IbcTimeoutBlock {
            revision: height.revision_number,
            height: height.revision_height,
        }),
        (seconds, None) => IbcTimeout::with_timestamp(Timestamp::from_nanos(seconds)),

        (seconds, Some(height)) => IbcTimeout::with_both(
            IbcTimeoutBlock {
                revision: height.revision_number,
                height: height.revision_height,
            },
            Timestamp::from_nanos(seconds),
        ),
    }
}
