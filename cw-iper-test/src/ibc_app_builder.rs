use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{testing::MockStorage, Empty, Storage};
use cw_multi_test::{
    AppBuilder, BankKeeper, DistributionKeeper, FailingModule, GovFailingModule, MockApiBech32,
    Module, StakeKeeper, WasmKeeper,
};

use crate::{
    ibc_application::IbcApplication,
    ibc_module::IbcModule,
    stargate::{StargateApplication, StargateModule},
};

pub struct IbcAppBuilder {}

impl IbcAppBuilder {
    #[allow(clippy::type_complexity)]
    #[allow(clippy::new_ret_no_self)]
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
        IbcModule,
        GovFailingModule,
        StargateModule,
    > {
        AppBuilder::new()
            .with_ibc(IbcModule::default())
            .with_stargate(StargateModule::default())
            .with_api(MockApiBech32::new(prefix))
    }
}

pub trait AppBuilderIbcExt: Sized {
    fn with_ibc_app<T: IbcApplication + StargateApplication + 'static>(
        self,
        application: T,
    ) -> Self;
}

impl<BankT, StorageT, CustomT: Module, WasmT, StakingT, DistrT, GovT> AppBuilderIbcExt
    for AppBuilder<
        BankT,
        MockApiBech32,
        StorageT,
        CustomT,
        WasmT,
        StakingT,
        DistrT,
        IbcModule,
        GovT,
        StargateModule,
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

pub trait AppBuilderStargateExt: Sized {
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
        StargateModule,
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
