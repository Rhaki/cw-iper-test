use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use anyhow::bail;
use cosmwasm_std::{
    Addr, AnyMsg, Api, Binary, BlockInfo, CustomMsg, CustomQuery, Empty, GrpcQuery, Querier,
    Storage,
};
use cw_multi_test::{AppResponse, CosmosRouter, Module, Stargate};
use serde::de::DeserializeOwned;

use crate::router::{RouterWrapper, UseRouter, UseRouterResponse};

use crate::{error::AppResult, router_closure};

use cosmwasm_std::from_json;

#[derive(Default)]
pub struct StargateModule {
    pub applications: BTreeMap<String, Rc<RefCell<dyn StargateApplication>>>,
}

impl StargateModule {
    fn get_application_by_msg_type_url(
        &self,
        type_url: String,
    ) -> AppResult<&Rc<RefCell<dyn StargateApplication>>> {
        for application in self.applications.values() {
            let a = application.borrow().is_msg_type_url(type_url.clone());
            if a {
                return Ok(application);
            }
        }
        bail!("application not found")
    }

    fn get_application_by_query_type_url(
        &self,
        type_url: String,
    ) -> AppResult<&Rc<RefCell<dyn StargateApplication>>> {
        for application in self.applications.values() {
            if application.borrow().is_query_type_url(type_url.clone()) {
                return Ok(application);
            }
        }
        bail!("application not found")
    }

    pub fn try_add_application(
        &mut self,
        application: Rc<RefCell<dyn StargateApplication>>,
    ) -> AppResult<()> {
        for type_url in application.borrow().type_urls() {
            for existing_application in self.applications.values() {
                if existing_application
                    .borrow()
                    .is_msg_type_url(type_url.clone())
                    || existing_application
                        .borrow()
                        .is_query_type_url(type_url.clone())
                {
                    bail!("Dupplicated type_url among applications: {}", type_url)
                }
            }
        }

        let name = application.borrow().stargate_name();

        self.applications.insert(name, application);

        Ok(())
    }
}

impl Module for StargateModule {
    type ExecT = AnyMsg;

    type QueryT = GrpcQuery;

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
        let application = self.get_application_by_msg_type_url(msg.type_url.clone())?;

        let rc_storage = Rc::new(RefCell::new(storage));

        application.borrow().stargate_msg(
            api,
            rc_storage.clone(),
            &RouterWrapper::new(&router_closure!(router, api, rc_storage, block)),
            block,
            sender,
            msg.type_url,
            msg.value,
        )
    }

    fn query(
        &self,
        api: &dyn Api,
        storage: &dyn Storage,
        querier: &dyn Querier,
        block: &BlockInfo,
        request: Self::QueryT,
    ) -> AppResult<Binary> {
        let application = self.get_application_by_query_type_url(request.path.clone())?;

        application
            .borrow()
            .stargate_query(api, storage, querier, block, request)
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
        unimplemented!("Stargate Sudo is unimplemented")
    }
}

impl Stargate for StargateModule {}

pub trait StargateApplication: StargateUrls {
    fn stargate_msg(
        &self,
        api: &dyn Api,
        storage: Rc<RefCell<&mut dyn Storage>>,
        router: &RouterWrapper,
        block: &BlockInfo,
        sender: Addr,
        type_url: String,
        data: Binary,
    ) -> AppResult<AppResponse>;

    fn stargate_query(
        &self,
        api: &dyn Api,
        storage: &dyn Storage,
        querier: &dyn Querier,
        block: &BlockInfo,
        request: GrpcQuery,
    ) -> AppResult<Binary>;
}

pub trait StargateUrls {
    fn stargate_name(&self) -> String;

    fn is_query_type_url(&self, type_url: String) -> bool;

    fn is_msg_type_url(&self, type_url: String) -> bool;

    fn type_urls(&self) -> Vec<String>;
}
