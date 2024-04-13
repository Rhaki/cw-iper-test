use std::{cell::RefCell, collections::BTreeMap, rc::Rc, u64};

use anyhow::anyhow;

use cosmwasm_std::{
    from_json, Addr, Api, Binary, BlockInfo, CustomMsg, CustomQuery, Empty, IbcChannelConnectMsg,
    IbcChannelOpenMsg, IbcMsg, IbcPacketAckMsg, IbcQuery, Querier, Storage,
};
use cw_multi_test::{AppResponse, CosmosRouter, Ibc, Module};
use cw_storage_plus::Item;
use serde::de::DeserializeOwned;

use crate::{
    error::AppResult,
    ibc::{IbcMsgExt, IbcPort},
    ibc_app::{PacketReceiveResponse, SharedChannels},
    ibc_applications::IbcApplication,
};

pub const PENDING_PACKETS: Item<BTreeMap<u64, IbcMsg>> = Item::new("pending_packets");

#[derive(Default)]
pub struct IbcModule {
    pub applications: BTreeMap<String, Box<dyn IbcApplication>>,
    pub channels: SharedChannels,
}

impl IbcModule {
    fn load_application(
        &self,
        name: impl Into<String> + Clone,
    ) -> AppResult<&Box<dyn IbcApplication>> {
        self.applications
            .get(&name.clone().into())
            .ok_or(anyhow!("application not found: {}", name.into()))
    }

    pub fn open_channel<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        application: &str,
        msg: IbcChannelOpenMsg,
    ) -> AppResult<AppResponse>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        let rc_storage = Rc::new(RefCell::new(storage));

        let clos = |action: UseRouter| {
            let res = match action {
                UseRouter::Query { b64_request } => {
                    let res =
                        router.query(api, *rc_storage.borrow(), block, from_json(b64_request)?)?;

                    UseRouterResponse::QueryResponse {
                        b64_response: res.into(),
                    }
                }
                UseRouter::ExecC {
                    b64_msg,
                    sender_msg,
                } => {
                    let res = router.execute(
                        api,
                        *rc_storage.borrow_mut(),
                        block,
                        sender_msg,
                        from_json(b64_msg)?,
                    )?;

                    UseRouterResponse::ExecCResponse { response: res }
                }
            };

            Ok(res)
        };

        self.load_application(application)?
            .open_channel(msg, api, block, &clos, rc_storage.clone())
    }

    pub fn channel_connect<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        application: &str,
        msg: IbcChannelConnectMsg,
    ) -> AppResult<AppResponse>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        let rc_storage = Rc::new(RefCell::new(storage));

        let clos = |action: UseRouter| {
            let res = match action {
                UseRouter::Query { b64_request } => {
                    let res =
                        router.query(api, *rc_storage.borrow(), block, from_json(b64_request)?)?;

                    UseRouterResponse::QueryResponse {
                        b64_response: res.into(),
                    }
                }
                UseRouter::ExecC {
                    b64_msg,
                    sender_msg,
                } => {
                    let res = router.execute(
                        api,
                        *rc_storage.borrow_mut(),
                        block,
                        sender_msg,
                        from_json(b64_msg)?,
                    )?;

                    UseRouterResponse::ExecCResponse { response: res }
                }
            };

            Ok(res)
        };

        self.load_application(application)?.channel_connect(
            msg,
            api,
            block,
            &clos,
            rc_storage.clone(),
        )
    }

    pub fn packet_receive<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        application: &str,
        msg: IbcMsg,
    ) -> AppResult<PacketReceiveResponse>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        let rc_storage = Rc::new(RefCell::new(storage));

        let clos = |action: UseRouter| {
            let res = match action {
                UseRouter::Query { b64_request } => {
                    let res =
                        router.query(api, *rc_storage.borrow(), block, from_json(b64_request)?)?;

                    UseRouterResponse::QueryResponse {
                        b64_response: res.into(),
                    }
                }
                UseRouter::ExecC {
                    b64_msg,
                    sender_msg,
                } => {
                    let res = router.execute(
                        api,
                        *rc_storage.borrow_mut(),
                        block,
                        sender_msg,
                        from_json(b64_msg)?,
                    )?;

                    UseRouterResponse::ExecCResponse { response: res }
                }
            };

            Ok(res)
        };

        self.load_application(application)?.packet_receive(
            msg,
            api,
            block,
            &clos,
            rc_storage.clone(),
        )
    }

    pub fn packet_ack<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        application: &str,
        msg: IbcPacketAckMsg,
    ) -> AppResult<AppResponse>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        let rc_storage = Rc::new(RefCell::new(storage));

        let clos = |action: UseRouter| {
            let res = match action {
                UseRouter::Query { b64_request } => {
                    let res =
                        router.query(api, *rc_storage.borrow(), block, from_json(b64_request)?)?;

                    UseRouterResponse::QueryResponse {
                        b64_response: res.into(),
                    }
                }
                UseRouter::ExecC {
                    b64_msg,
                    sender_msg,
                } => {
                    let res = router.execute(
                        api,
                        *rc_storage.borrow_mut(),
                        block,
                        sender_msg,
                        from_json(b64_msg)?,
                    )?;

                    UseRouterResponse::ExecCResponse { response: res }
                }
            };

            Ok(res)
        };

        self.load_application(application)?
            .packet_ack(msg, api, block, &clos, rc_storage.clone())
    }
}

impl Module for IbcModule {
    type ExecT = IbcMsg;
    type QueryT = IbcQuery;
    type SudoT = Empty;

    fn execute<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        sender: Addr,
        msg: Self::ExecT,
    ) -> AppResult<AppResponse>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        let channel = self.channels.borrow().get(msg.get_src_channel())?.clone();

        let rc_storage = Rc::new(RefCell::new(storage));

        let clos = |action: UseRouter| {
            let res = match action {
                UseRouter::Query { b64_request } => {
                    let res =
                        router.query(api, *rc_storage.borrow(), block, from_json(b64_request)?)?;

                    UseRouterResponse::QueryResponse {
                        b64_response: res.into(),
                    }
                }
                UseRouter::ExecC {
                    b64_msg,
                    sender_msg,
                } => {
                    let res = router.execute(
                        api,
                        *rc_storage.borrow_mut(),
                        block,
                        sender_msg,
                        from_json(b64_msg)?,
                    )?;

                    UseRouterResponse::ExecCResponse { response: res }
                }
            };

            Ok(res)
        };

        let response = match &msg {
            IbcMsg::CloseChannel { .. } => todo!(),
            _ => {
                if let IbcPort::Module(name) = &channel.local.port {
                    self.load_application(name)?.handle_outgoing_packet(
                        msg.clone(),
                        api,
                        block,
                        sender,
                        &clos,
                        rc_storage.clone(),
                    )?
                } else {
                    AppResponse::default()
                }
            }
        };

        let mut packets = PENDING_PACKETS
            .load(*rc_storage.borrow())
            .unwrap_or_default();
        let new_key = packets.last_key_value().map(|(k, _)| *k).unwrap_or(0) + 1;
        packets.insert(new_key, msg);
        PENDING_PACKETS.save(*rc_storage.borrow_mut(), &packets)?;
        Ok(response)
    }

    fn query(
        &self,
        _api: &dyn Api,
        _storage: &dyn Storage,
        _querier: &dyn Querier,
        _block: &BlockInfo,
        _request: Self::QueryT,
    ) -> AppResult<Binary> {
        todo!()
    }

    fn sudo<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        _msg: Self::SudoT,
    ) -> AppResult<AppResponse>
    where
        ExecC: CustomMsg + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        todo!()
    }
}

impl Ibc for IbcModule {}

pub enum UseRouter {
    Query { b64_request: Binary },
    ExecC { b64_msg: Binary, sender_msg: Addr },
}

pub enum UseRouterResponse {
    QueryResponse { b64_response: Binary },
    ExecCResponse { response: AppResponse },
}
