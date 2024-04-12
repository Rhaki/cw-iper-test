use cosmwasm_std::{
    testing::{MockApi, MockStorage},
    Empty,
};
use cw_multi_test::{
    AppBuilder, BankKeeper, DistributionKeeper, FailingModule, GovFailingModule, StakeKeeper,
    StargateFailingModule, WasmKeeper,
};

use crate::ibc_module::IbcModule;

pub struct IbcAppBuilder {}

impl IbcAppBuilder {
    pub fn new() -> AppBuilder<
        BankKeeper,
        MockApi,
        MockStorage,
        FailingModule<Empty, Empty, Empty>,
        WasmKeeper<Empty, Empty>,
        StakeKeeper,
        DistributionKeeper,
        IbcModule,
        GovFailingModule,
        StargateFailingModule,
    > {
        AppBuilder::new().with_ibc(IbcModule::default())
    }
}
