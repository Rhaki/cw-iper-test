use cosmwasm_std::{Api, CustomMsg, CustomQuery, Storage};
use cw_multi_test::{App, Bank, Distribution, Gov, Ibc, Module, Staking, Stargate, Wasm};
use serde::de::DeserializeOwned;

use crate::ibc_app::IbcApp;

pub trait AppExt<
    BankT,
    ApiT,
    StorageT,
    CustomT: Module,
    WasmT,
    StakingT,
    DistrT,
    IbcT,
    GovT,
    StargateT,
> where
    CustomT::QueryT: CustomQuery,
{
    fn into_ibc_app(
        self,
    ) -> IbcApp<BankT, ApiT, StorageT, CustomT, WasmT, StakingT, DistrT, IbcT, GovT, StargateT>;
}

impl<BankT, ApiT, StorageT, CustomT: Module, WasmT, StakingT, DistrT, IbcT, GovT, StargateT>
    AppExt<BankT, ApiT, StorageT, CustomT, WasmT, StakingT, DistrT, IbcT, GovT, StargateT>
    for App<BankT, ApiT, StorageT, CustomT, WasmT, StakingT, DistrT, IbcT, GovT, StargateT>
where
    CustomT::QueryT: CustomQuery + DeserializeOwned + 'static,
    CustomT::ExecT: CustomMsg + DeserializeOwned + 'static,
    WasmT: Wasm<CustomT::ExecT, CustomT::QueryT>,
    BankT: Bank,
    ApiT: Api,
    StorageT: Storage,
    CustomT: Module,
    StakingT: Staking,
    DistrT: Distribution,
    IbcT: Ibc,
    GovT: Gov,
    StargateT: Stargate,
{
    fn into_ibc_app(
        self,
    ) -> IbcApp<BankT, ApiT, StorageT, CustomT, WasmT, StakingT, DistrT, IbcT, GovT, StargateT>
    {
        IbcApp {
            app: self,
            code_ids: Default::default(),
            channels: Default::default(),
        }
    }
}
