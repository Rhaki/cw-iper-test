use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use cosmwasm_std::{Addr, Api, CustomMsg, CustomQuery, IbcMsg, Storage};
use cw_multi_test::{App, Bank, Distribution, Gov, Module, Staking, Stargate, Wasm};
use serde::de::DeserializeOwned;

use crate::{
    contracts::MultiContract,
    error::AppResult,
    ibc_app::IbcApp,
    ibc_module::{IbcChannelCreator, IbcChannelWrapper, IbcModule, PENDING_PACKETS},
    response::RelayedResponse,
};

#[rustfmt::skip]
pub struct IperApp<BankT1, ApiT1, StorageT1, CustomT1: Module, WasmT1, StakingT1, DistrT1,  GovT1, StargateT1, BankT2, ApiT2, StorageT2, CustomT2: Module, WasmT2, StakingT2, DistrT2,  GovT2, StargateT2>
where
{
    app_1: IbcApp<BankT1, ApiT1, StorageT1, CustomT1, WasmT1, StakingT1, DistrT1, IbcModule, GovT1, StargateT1>,
    app_2: IbcApp<BankT2, ApiT2, StorageT2, CustomT2, WasmT2, StakingT2, DistrT2, IbcModule, GovT2, StargateT2>,
}

// #[rustfmt::skip]
impl<
        BankT1,
        ApiT1,
        StorageT1,
        CustomT1,
        WasmT1,
        StakingT1,
        DistrT1,
        GovT1,
        StargateT1,
        BankT2,
        ApiT2,
        StorageT2,
        CustomT2,
        WasmT2,
        StakingT2,
        DistrT2,
        GovT2,
        StargateT2,
    >
    IperApp<
        BankT1,
        ApiT1,
        StorageT1,
        CustomT1,
        WasmT1,
        StakingT1,
        DistrT1,
        GovT1,
        StargateT1,
        BankT2,
        ApiT2,
        StorageT2,
        CustomT2,
        WasmT2,
        StakingT2,
        DistrT2,
        GovT2,
        StargateT2,
    >
where
    WasmT1: Wasm<CustomT1::ExecT, CustomT1::QueryT>,
    BankT1: Bank,
    ApiT1: Api,
    StorageT1: Storage,
    CustomT1: Module,
    StakingT1: Staking,
    DistrT1: Distribution,
    GovT1: Gov,
    StargateT1: Stargate,
    CustomT1::ExecT: CustomMsg + DeserializeOwned + 'static,
    CustomT1::QueryT: CustomQuery + DeserializeOwned + 'static,
    WasmT2: Wasm<CustomT2::ExecT, CustomT2::QueryT>,
    BankT2: Bank,
    ApiT2: Api,
    StorageT2: Storage,
    CustomT2: Module,
    StakingT2: Staking,
    DistrT2: Distribution,
    GovT2: Gov,
    StargateT2: Stargate,
    CustomT2::ExecT: CustomMsg + DeserializeOwned + 'static,
    CustomT2::QueryT: CustomQuery + DeserializeOwned + 'static,
{
    pub fn new(
        app_1: IbcApp<
            BankT1,
            ApiT1,
            StorageT1,
            CustomT1,
            WasmT1,
            StakingT1,
            DistrT1,
            IbcModule,
            GovT1,
            StargateT1,
        >,
        app_2: IbcApp<
            BankT2,
            ApiT2,
            StorageT2,
            CustomT2,
            WasmT2,
            StakingT2,
            DistrT2,
            IbcModule,
            GovT2,
            StargateT2,
        >,
    ) -> Self {
        IperApp { app_1, app_2 }
    }

    pub fn store_code_on_1(
        &mut self,
        contract: MultiContract<CustomT1::ExecT, CustomT1::QueryT>,
    ) -> u64 {
        let code_id = self.app_1.app.store_code(contract.base);
        if let Some(ibc) = contract.ibc {
            self.app_1.code_ids.insert(code_id, ibc);
        }
        code_id
    }

    pub fn store_code_on_2(
        &mut self,
        contract: MultiContract<CustomT2::ExecT, CustomT2::QueryT>,
    ) -> u64 {
        let code_id = self.app_2.app.store_code(contract.base);
        if let Some(ibc) = contract.ibc {
            self.app_2.code_ids.insert(code_id, ibc);
        }
        code_id
    }

    pub fn app_1_mut(
        &mut self,
    ) -> &mut App<
        BankT1,
        ApiT1,
        StorageT1,
        CustomT1,
        WasmT1,
        StakingT1,
        DistrT1,
        IbcModule,
        GovT1,
        StargateT1,
    > {
        &mut self.app_1.app
    }

    pub fn app_2_mut(
        &mut self,
    ) -> &mut App<
        BankT2,
        ApiT2,
        StorageT2,
        CustomT2,
        WasmT2,
        StakingT2,
        DistrT2,
        IbcModule,
        GovT2,
        StargateT2,
    > {
        &mut self.app_2.app
    }

    pub fn app_1(
        &self,
    ) -> &App<
        BankT1,
        ApiT1,
        StorageT1,
        CustomT1,
        WasmT1,
        StakingT1,
        DistrT1,
        IbcModule,
        GovT1,
        StargateT1,
    > {
        &self.app_1.app
    }

    pub fn app_2(
        &self,
    ) -> &App<
        BankT2,
        ApiT2,
        StorageT2,
        CustomT2,
        WasmT2,
        StakingT2,
        DistrT2,
        IbcModule,
        GovT2,
        StargateT2,
    > {
        &self.app_2.app
    }

    pub fn pending_packets(&self) -> (BTreeMap<u64, IbcMsg>, BTreeMap<u64, IbcMsg>) {
        let pending_1 = PENDING_PACKETS
            .load(self.app_1.app.storage())
            .unwrap_or_default();
        let pending_2 = PENDING_PACKETS
            .load(self.app_2.app.storage())
            .unwrap_or_default();

        (pending_1, pending_2)
    }

    pub fn channels(
        &self,
    ) -> (
        BTreeMap<u64, IbcChannelWrapper>,
        BTreeMap<u64, IbcChannelWrapper>,
    ) {
        (self.app_1.channels.clone(), self.app_2.channels.clone())
    }

    pub fn relay_all_packets(&mut self, relayer: Addr) -> AppResult<Vec<RelayedResponse>> {
        let mut res = vec![];

        loop {
            if self.app_1.some_pending_packets() {
                res.push(self.relay_next_packet_1(relayer.clone())?)
            } else if self.app_2.some_pending_packets() {
                res.push(self.relay_next_packet_2(relayer.clone())?)
            } else {
                break;
            }
        }

        Ok(res)
    }

    pub fn relay_packet_1(&mut self, packet_id: u64, relayer: Addr) -> AppResult<RelayedResponse> {
        let msg = self.app_1.get_pending_packet(packet_id)?;

        let dest_channel_id = self.app_1.get_dest_channel_from_msg(&msg)?;

        let dest_response = self.app_2.packet_receive(&msg, &relayer, dest_channel_id)?;

        let ack_response = if let Some(ack) = &dest_response.ack {
            Some(self.app_1.packet_ack(ack.clone(), &msg, &relayer)?)
        } else {
            None
        };

        self.app_1.remove_packet(packet_id)?;

        Ok(RelayedResponse {
            relayer,
            msg,
            dest_response: dest_response.response,
            ack: dest_response.ack,
            src_response: ack_response,
        })
    }

    pub fn relay_packet_2(&mut self, packet_id: u64, relayer: Addr) -> AppResult<RelayedResponse> {
        let msg = self.app_2.get_pending_packet(packet_id)?;

        let dest_channel_id = self.app_2.get_dest_channel_from_msg(&msg)?;

        let dest_response = self.app_1.packet_receive(&msg, &relayer, dest_channel_id)?;

        let ack_response = if let Some(ack) = &dest_response.ack {
            Some(self.app_2.packet_ack(ack.clone(), &msg, &relayer)?)
        } else {
            None
        };

        self.app_2.remove_packet(packet_id)?;

        Ok(RelayedResponse {
            relayer,
            msg,
            dest_response: dest_response.response,
            ack: dest_response.ack,
            src_response: ack_response,
        })
    }

    pub fn relay_next_packet_1(&mut self, relayer: Addr) -> AppResult<RelayedResponse> {
        let packet_id = self.app_1.get_next_pending_packet()?;
        self.relay_packet_1(packet_id, relayer)
    }

    pub fn relay_next_packet_2(&mut self, relayer: Addr) -> AppResult<RelayedResponse> {
        let packet_id = self.app_2.get_next_pending_packet()?;
        self.relay_packet_2(packet_id, relayer)
    }

    pub fn open_ibc_channel(
        &mut self,
        mut channel_1: IbcChannelCreator,
        mut channel_2: IbcChannelCreator,
    ) -> AppResult<()> {
        let channel_id_1 = self.app_1.get_next_channel_id();
        let channel_id_2 = self.app_2.get_next_channel_id();
        channel_1.set_channel_id(channel_id_1);
        channel_2.set_channel_id(channel_id_2);

        // Open
        let sequence = Rc::new(RefCell::new(0));
        self.app_1
            .channel_open(&channel_1, &channel_2, sequence.clone())?;
        self.app_2.channel_open(&channel_2, &channel_1, sequence)?;

        // Connect Ack

        self.app_1.channel_connect(channel_id_1)?;
        self.app_2.channel_connect(channel_id_2)?;

        // Connect Confirm

        self.app_1.channel_connect(channel_id_1)?;
        self.app_2.channel_connect(channel_id_2)?;

        Ok(())
    }
}
