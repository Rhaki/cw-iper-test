use std::{cell::RefCell, collections::BTreeMap, rc::Rc, u64};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Empty, IbcChannel, IbcEndpoint, IbcMsg, IbcOrder, IbcQuery};
use cw_multi_test::{AppResponse, Ibc, Module};
use cw_storage_plus::Item;

use anyhow::{anyhow, bail};

use crate::error::AppResult;
#[derive(Default)]
pub struct IbcModule {}

pub const PENDING_PACKETS: Item<BTreeMap<u64, IbcMsg>> = Item::new("pending_packets");

impl Module for IbcModule {
    type ExecT = IbcMsg;

    type QueryT = IbcQuery;

    type SudoT = Empty;

    fn execute<ExecC, QueryC>(
        &self,
        _api: &dyn cosmwasm_std::Api,
        storage: &mut dyn cosmwasm_std::Storage,
        _router: &dyn cw_multi_test::CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &cosmwasm_std::BlockInfo,
        _sender: cosmwasm_std::Addr,
        msg: Self::ExecT,
    ) -> anyhow::Result<cw_multi_test::AppResponse>
    where
        ExecC: cosmwasm_std::CustomMsg + serde::de::DeserializeOwned + 'static,
        QueryC: cosmwasm_std::CustomQuery + serde::de::DeserializeOwned + 'static,
    {
        let mut packets = PENDING_PACKETS.load(storage).unwrap_or_default();
        let new_key = packets
            .last_key_value()
            .map(|(k, _)| k.clone())
            .unwrap_or(0)
            + 1;
        packets.insert(new_key, msg);
        PENDING_PACKETS.save(storage, &packets)?;
        Ok(AppResponse::default())
    }

    fn query(
        &self,
        _api: &dyn cosmwasm_std::Api,
        _storage: &dyn cosmwasm_std::Storage,
        _querier: &dyn cosmwasm_std::Querier,
        _block: &cosmwasm_std::BlockInfo,
        _request: Self::QueryT,
    ) -> anyhow::Result<cosmwasm_std::Binary> {
        todo!()
    }

    fn sudo<ExecC, QueryC>(
        &self,
        _api: &dyn cosmwasm_std::Api,
        _storage: &mut dyn cosmwasm_std::Storage,
        _router: &dyn cw_multi_test::CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &cosmwasm_std::BlockInfo,
        _msg: Self::SudoT,
    ) -> anyhow::Result<cw_multi_test::AppResponse>
    where
        ExecC: cosmwasm_std::CustomMsg + serde::de::DeserializeOwned + 'static,
        QueryC: cosmwasm_std::CustomQuery + serde::de::DeserializeOwned + 'static,
    {
        todo!()
    }
}

impl Ibc for IbcModule {}

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

#[cw_serde]
pub enum IbcChannelStatus {
    Created,
    Opening,
    Connected,
    Closed,
}

impl IbcChannelStatus {
    pub fn next(&mut self) -> AppResult<()> {
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
}

pub trait IbcChannelBuilder {
    fn as_channel_string(&self) -> String;
}

impl IbcChannelBuilder for u64 {
    fn as_channel_string(&self) -> String {
        format!("channel-{}", self)
    }
}

pub trait IbcChannelDebuilder {
    fn as_channel_number(&self) -> AppResult<u64>;
}

impl IbcChannelDebuilder for String {
    fn as_channel_number(&self) -> AppResult<u64> {
        self.strip_prefix("channel-")
            .ok_or(anyhow!("invalid channel id"))
            .and_then(|s| s.parse::<u64>().map_err(|_| anyhow!("invalid channel id")))
    }
}

#[cw_serde]
#[non_exhaustive]
pub struct IbcChannelCreator {
    pub port: IbcPort,
    pub order: IbcOrder,
    pub version: String,
    pub connection_id: String,
    channel_id: Option<u64>,
}

impl IbcChannelCreator {
    pub fn new(
        port: IbcPort,
        order: IbcOrder,
        version: impl Into<String>,
        connection_id: impl Into<String>,
    ) -> Self {
        Self {
            port,
            order,
            version: version.into(),
            connection_id: connection_id.into(),
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
}
