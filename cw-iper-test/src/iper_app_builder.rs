use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{testing::MockStorage, Empty, Storage};
use cw_multi_test::{
    AppBuilder, BankKeeper, DistributionKeeper, FailingModule, GovFailingModule, MockApiBech32,
    Module, StakeKeeper, WasmKeeper,
};

use crate::{
    ibc_application::IbcApplication,
    ibc_module::IperIbcModule,
    stargate::{IperStargateModule, StargateApplication},
};

/// Shorthcut of [`AppBuilder`] version for [`IperApp`](crate::iper_app::IperApp) that create an [`AppBuilder`] with:
/// - `api`:  [`MockApiBech32`];
/// - `ibc`: [`IperIbcModule`];
/// - `stargate`: [`IperStargateModule`].
///
/// Calling [`IperAppBuilder::new`] is equal to create an [`AppBuilder`] with:
/// ```ignore
/// AppBuilder::new()
///     .with_ibc(IperIbcModule::default())
///     .with_stargate(IperStargateModule::default())
///     .with_api(MockApiBech32::new("prefix"))
pub struct IperAppBuilder;

impl IperAppBuilder {
    #[allow(clippy::type_complexity)]
    #[allow(clippy::new_ret_no_self)]
    /// Create a new [`AppBuilder`] as:
    /// ```ignore
    /// AppBuilder::new()
    ///     .with_ibc(IperIbcModule::default())
    ///     .with_stargate(IperStargateModule::default())
    ///     .with_api(MockApiBech32::new("prefix"))
    pub fn new(
        prefix: &'static str,
    ) -> AppBuilder<
        BankKeeper,
        MockApiBech32,
        MockStorage,
        FailingModule<Empty, Empty, Empty>,
        WasmKeeper<Empty, Empty>,
        StakeKeeper,
        DistributionKeeper,
        IperIbcModule,
        GovFailingModule,
        IperStargateModule,
    > {
        AppBuilder::new()
            .with_ibc(IperIbcModule::default())
            .with_stargate(IperStargateModule::default())
            .with_api(MockApiBech32::new(prefix))
    }
}

/// Trait implemented in [`AppBuilder`] where:
/// - `api`:  [`MockApiBech32`];
/// - `ibc`: [`IperIbcModule`];
/// - `stargate`: [`IperStargateModule`].
///
/// The function [`AppBuilderIperExt::with_ibc_app`] allow to insert a struct that implement both [`IbcApplication`] + [StargateApplication] inside [`IperIbcModule`] and [`IperStargateModule`].
pub trait AppBuilderIperExt: Sized {
    /// insert a struct that implement both [`IbcApplication`] + [StargateApplication] inside the [`IperIbcModule`] and [`IperStargateModule`].
    fn with_ibc_app<T: IbcApplication + StargateApplication + 'static>(
        self,
        application: T,
    ) -> Self;
}

impl<BankT, StorageT, CustomT: Module, WasmT, StakingT, DistrT, GovT> AppBuilderIperExt
    for AppBuilder<
        BankT,
        MockApiBech32,
        StorageT,
        CustomT,
        WasmT,
        StakingT,
        DistrT,
        IperIbcModule,
        GovT,
        IperStargateModule,
    >
where
    StorageT: Storage,
{
    fn with_ibc_app<T: IbcApplication + StargateApplication + 'static>(
        mut self,
        application: T,
    ) -> Self {
        let mut ibc = self.ibc;
        let mut stargate = self.stargate;
        application.init(&self.api, &mut self.storage);
        let application = Rc::new(RefCell::new(application));
        let port_name = application.borrow().port_name();

        ibc.applications.insert(port_name, application.clone());
        stargate.try_add_application(application).unwrap();

        Self {
            api: self.api,
            block: self.block,
            storage: self.storage,
            bank: self.bank,
            wasm: self.wasm,
            custom: self.custom,
            staking: self.staking,
            distribution: self.distribution,
            ibc,
            gov: self.gov,
            stargate,
        }
    }
}

/// Trait implemented in [`AppBuilder`] where:
/// - `api`:  [`MockApiBech32`];
/// - `stargate`: [`IperStargateModule`].
///
/// The function [`AppBuilderIperExt::with_stargate_app`] allow to insert a struct that implement [StargateApplication] inside [`IperStargateModule`].

pub trait AppBuilderStargateExt: Sized {
    /// Insert a struct that implement [StargateApplication] inside [`IperStargateModule`].
    fn with_stargate_app<T: StargateApplication + 'static>(self, application: T) -> Self;
}

impl<BankT, StorageT, CustomT: Module, WasmT, StakingT, DistrT, IbcT, GovT> AppBuilderStargateExt
    for AppBuilder<
        BankT,
        MockApiBech32,
        StorageT,
        CustomT,
        WasmT,
        StakingT,
        DistrT,
        IbcT,
        GovT,
        IperStargateModule,
    >
where
    StorageT: Storage,
{
    fn with_stargate_app<T: StargateApplication + 'static>(self, application: T) -> Self {
        let mut stargate = self.stargate;
        let application = Rc::new(RefCell::new(application));
        stargate.try_add_application(application.clone()).unwrap();

        Self {
            api: self.api,
            block: self.block,
            storage: self.storage,
            bank: self.bank,
            wasm: self.wasm,
            custom: self.custom,
            staking: self.staking,
            distribution: self.distribution,
            ibc: self.ibc,
            gov: self.gov,
            stargate,
        }
    }
}
