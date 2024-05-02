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

/// The [`IperStargateModule`] is the default struct used in an [`IperApp`](crate::iper_app::IperApp) as an `Stargate module` and contains all [`StargateApplication`].
///
/// This structure implements the [`Module`] and [`Stargate`] `traits` from `cw-multi-test`.
///
/// When an [`AnyMsg`] needs to be handled, the [`IperStargateModule`] attempts to find a
/// [`StargateApplication`] that manages the `type_url` of the [`AnyMsg`] and call [`StargateApplication::stargate_msg`].
///
/// [`StargateApplication`] instances must be added to the [`IperStargateModule`] during the creation of the [`App`](cw_multi_test::App)
/// via the [`AppBuilder`](cw_multi_test::AppBuilder). This is achieved using one of the following function:
/// - **[`AppBuilderIperExt::with_ibc_app`](crate::iper_app_builder::AppBuilderIperExt)** if the [`StargateApplication`] **is also** [`IbcApplication`](crate::ibc_application::IbcApplication);
/// - **[`AppBuilderStargateExt::with_stargate_app`](crate::iper_app_builder::AppBuilderStargateExt)** if the [`StargateApplication`] **is not** [`IbcApplication`](crate::ibc_application::IbcApplication).
///
/// It is essential that the `Stargate module` in the [`AppBuilder`](cw_multi_test::AppBuilder) is set to [`IperStargateModule`] for this integration
/// to function correctly.
#[derive(Default)]
pub struct IperStargateModule {
    applications: BTreeMap<String, Rc<RefCell<dyn StargateApplication>>>,
}

impl IperStargateModule {
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

    /// Try add a StargateApplication.
    ///
    /// If another StargateApplication alredy implements one of the
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

impl Module for IperStargateModule {
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

impl Stargate for IperStargateModule {}

/// This trait identifies a generic `Stargate application`(e.g., `TokenFactory`, [`IC20`](crate::ibc_applications::Ics20)) that is managed by the [`IperStargateModule`](crate::stargate::IperStargateModule).
///
/// The [`IperStargateModule`] is a structure implementing both [`Stargate`](cw_multi_test::Stargate) and [`Module`](cw_multi_test::Module)
/// traits and serves as the `Stargate module` for the [`App`](cw_multi_test::App) class of an [`IperApp`](crate::IperApp).
///
/// [`IperStargateModule`] will invoke a function implemented by this trait under the following conditions:
///
/// - **stargate_msg**: A [`AnyMsg`] is targetting this [`StargateApplication`] for execution;
/// - **stargate_query**: A [`GrpcQuery`] is targetting this [`StargateApplication`] for query.
///
/// ## Implementation of the trait:
/// In order to be implemented, the struct has to implement both [`StargateUrls`] + [`StargateName`]
///
/// Use the `derive macro` `Stargate` and #[urls] `proc_macro_attribute` `from cw-iper-test-macros`
/// ## Example:
/// ```ignore
/// #[derive(Stargate)]
/// #[stargate(name = "ics20", query_urls = Ics20QueryUrls, msgs_urls = Ics20MsgUrls)]
/// pub struct Ics20;
///
/// #[urls]
/// pub enum Ics20MsgUrls {
///     #[strum(serialize = "/ibc.applications.transfer.v1.MsgTransfer")]
///     MsgTransfer,
/// }
/// #[urls]
/// pub enum Ics20QueryUrls {}
///
pub trait StargateApplication: StargateUrls + StargateName {
    /// A [`AnyMsg`] is targetting this [`StargateApplication`] for execution;
    #[allow(clippy::too_many_arguments)]
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

    /// A [`GrpcQuery`] is targetting this [`StargateApplication`] for query.
    fn stargate_query(
        &self,
        api: &dyn Api,
        storage: &dyn Storage,
        querier: &dyn Querier,
        block: &BlockInfo,
        request: GrpcQuery,
    ) -> AppResult<Binary>;
}

/// Trait required by [`StargateApplication`] for the managment of `type_urls`.
///
/// This shouldn't be implemented directly into a [`StargateApplication`], but instead using the derive macro` `Stargate` and #[urls] `proc_macro_attribute` `from cw-iper-test-macros`.
pub trait StargateUrls {
    /// Check if a type_url of a [`GrpcQuery`] is handled by the [`StargateApplication`].
    fn is_query_type_url(&self, type_url: String) -> bool;
    /// Check if a type_url of a [`AnyMsg`] is handled by the [`StargateApplication`].
    fn is_msg_type_url(&self, type_url: String) -> bool;
    /// Return all `type_urls`
    fn type_urls(&self) -> Vec<String>;
}

/// Trait required by [`StargateApplication`].
pub trait StargateName {
    /// Return the name the [`StargateApplication`].
    fn stargate_name(&self) -> String;
}
