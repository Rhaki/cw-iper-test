use cosmwasm_std::{testing::MockStorage, Empty};
use cw_multi_test::{
    AppBuilder, BankKeeper, DistributionKeeper, FailingModule, GovFailingModule, MockApiBech32,
    Module, StakeKeeper, StargateFailingModule, WasmKeeper,
};

use crate::{ibc_applications::IbcApplication, ibc_module::IbcModule};

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
        StargateFailingModule,
    > {
        AppBuilder::new()
            .with_ibc(IbcModule::default())
            .with_api(MockApiBech32::new(prefix))
    }
}

pub trait AppBuilderExt<BankT, StorageT, CustomT: Module, WasmT, StakingT, DistrT, GovT, StargateT>
{
    fn with_ibc_app<T: IbcApplication + 'static>(self, application: T) -> Self;
}

impl<BankT, StorageT, CustomT: Module, WasmT, StakingT, DistrT, GovT, StargateT>
    AppBuilderExt<BankT, StorageT, CustomT, WasmT, StakingT, DistrT, GovT, StargateT>
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
        StargateT,
    >
{
    fn with_ibc_app<T: IbcApplication + 'static>(self, application: T) -> Self {
        let mut ibc = self.ibc;
        ibc.applications
            .insert(application.port_name(), Box::new(application));

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
            stargate: self.stargate,
        }
    }
}
