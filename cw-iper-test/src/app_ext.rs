use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{CustomMsg, CustomQuery, Storage};
use cw_multi_test::{App, Bank, Distribution, Gov, MockApiBech32, Module, Staking, Stargate, Wasm};
use serde::de::DeserializeOwned;

use crate::{
    chain_helper::ChainHelper,
    ibc_app::{IbcApp, SharedChannels},
    ibc_module::IbcModule,
};

pub trait AppExt<
    BankT,
    MockApiBech32,
    StorageT,
    CustomT: Module,
    WasmT,
    StakingT,
    DistrT,
    GovT,
    StargateT,
> where
    CustomT::QueryT: CustomQuery,
{
    fn into_ibc_app(
        self,
        chain_id: impl Into<String>,
    ) -> Rc<
        RefCell<
            IbcApp<
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
            >,
        >,
    >;
}

impl<BankT, StorageT, CustomT: Module, WasmT, StakingT, DistrT, GovT, StargateT>
    AppExt<BankT, MockApiBech32, StorageT, CustomT, WasmT, StakingT, DistrT, GovT, StargateT>
    for App<
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
where
    CustomT::QueryT: CustomQuery + DeserializeOwned + 'static,
    CustomT::ExecT: CustomMsg + DeserializeOwned + 'static,
    WasmT: Wasm<CustomT::ExecT, CustomT::QueryT>,
    BankT: Bank,
    StorageT: Storage,
    CustomT: Module,
    StakingT: Staking,
    DistrT: Distribution,
    GovT: Gov,
    StargateT: Stargate,
{
    fn into_ibc_app(
        mut self,
        chain_id: impl Into<String>,
    ) -> Rc<
        RefCell<
            IbcApp<
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
            >,
        >,
    > {
        let channels: SharedChannels = self.read_module(|router, _, _| router.ibc.channels.clone());

        let chain_prefix = self.api().prefix().to_string();
        ChainHelper { chain_prefix }
            .save(self.storage_mut())
            .unwrap();

        Rc::new(RefCell::new(IbcApp {
            relayer: self.api().addr_make("default_relayer"),
            chain_id: chain_id.into(),
            app: self,
            code_ids: Default::default(),
            channels,
        }))
    }
}
