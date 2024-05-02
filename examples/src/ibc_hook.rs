use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::{Addr, Coin, CosmosMsg, IbcMsg, IbcOrder, IbcTimeout, Timestamp, Uint128};
use cw_iper_test::{
    cw_multi_test::{no_init, BankSudo, ContractWrapper, Executor, SudoMsg},
    ibc_applications::{IbcHook, Ics20, Ics20Helper, MemoField, WasmField},
    AppBuilderIperExt, AppExt, BaseIperApp, ContractWrapperExt, Ecosystem, IbcChannelCreator,
    IbcPort, IperAppBuilder, MultiContract,
};

use crate::mock_contracts::counter::{self, CounterConfig, CounterQueryMsg};

struct TestIbcHookEnv {
    pub eco: Ecosystem,
    pub terra: Rc<RefCell<BaseIperApp>>,
    pub osmosis: Rc<RefCell<BaseIperApp>>,
    pub contract_terra: Addr,
    pub contract_osmosis: Addr,
}

fn startup() -> TestIbcHookEnv {
    let osmosis = IperAppBuilder::new("osmo")
        .with_ibc_app(IbcHook::new(Ics20))
        .build(no_init)
        .into_iper_app("osmosis");

    let terra = IperAppBuilder::new("terra")
        .with_ibc_app(IbcHook::new(Ics20))
        .build(no_init)
        .into_iper_app("terra");

    let eco = Ecosystem::default()
        .add_app(terra.clone())
        .add_app(osmosis.clone());

    let contract = MultiContract::new(
        ContractWrapper::new(counter::execute, counter::instantiate, counter::query)
            .with_sudo(counter::sudo)
            .to_contract(),
        None,
    );

    let code_id_osmosis = osmosis.borrow_mut().store_ibc_code(contract);

    let osmosis_owner = osmosis.borrow().app.api().addr_make("owner");

    let osmosis_contract_addr = osmosis
        .borrow_mut()
        .app
        .instantiate_contract(
            code_id_osmosis,
            osmosis_owner,
            &counter::InstantiateMsg {},
            &[],
            "label".to_string(),
            None,
        )
        .unwrap();

    let contract = MultiContract::new(
        ContractWrapper::new(counter::execute, counter::instantiate, counter::query)
            .with_sudo(counter::sudo)
            .to_contract(),
        None,
    );

    let code_id_terra = terra.borrow_mut().store_ibc_code(contract);

    let terra_owner = terra.borrow().app.api().addr_make("owner");

    let terra_contract_addr = terra
        .borrow_mut()
        .app
        .instantiate_contract(
            code_id_terra,
            terra_owner,
            &counter::InstantiateMsg {},
            &[],
            "label".to_string(),
            None,
        )
        .unwrap();

    eco.open_ibc_channel(
        IbcChannelCreator::new(
            IbcPort::from_application(Ics20),
            IbcOrder::Unordered,
            "version",
            "connection_id",
            "terra",
        ),
        IbcChannelCreator::new(
            IbcPort::from_application(Ics20),
            IbcOrder::Unordered,
            "version",
            "connection_id",
            "osmosis",
        ),
    )
    .unwrap();

    TestIbcHookEnv {
        eco,
        terra,
        osmosis,
        contract_terra: terra_contract_addr,
        contract_osmosis: osmosis_contract_addr,
    }
}

#[test]
fn ibc_hook_base() {
    let TestIbcHookEnv {
        eco,
        terra,
        osmosis,
        contract_osmosis,
        ..
    } = startup();

    let sender = terra.borrow().app.api().addr_make("sender");
    let receiver = osmosis.borrow().app.api().addr_make("receiver");

    let amount = Coin::new(1_000_000_u128, "uluna");

    let msg = CosmosMsg::Ibc(IbcMsg::Transfer {
        channel_id: "channel-0".to_string(),
        to_address: receiver.to_string(),
        amount: amount.clone(),
        timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(
            osmosis.borrow().app.block_info().time.seconds() + 1,
        )),
        memo: Some(
            serde_json::to_string_pretty(&MemoField::new(
                Some(WasmField {
                    contract: contract_osmosis.to_string(),
                    msg: counter::ExecuteMsg::JustReceive {
                        msg: "test".to_string(),
                        to_fail: false,
                    },
                }),
                None,
            ))
            .unwrap(),
        ),
    });

    terra
        .borrow_mut()
        .app
        .execute(sender.clone(), msg.clone())
        .unwrap_err();

    terra
        .borrow_mut()
        .app
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: sender.to_string(),
            amount: vec![amount.clone()],
        }))
        .unwrap();

    terra.borrow_mut().app.execute(sender.clone(), msg).unwrap();

    eco.relay_all_packets().unwrap();

    let balance = terra
        .borrow()
        .app
        .wrap()
        .query_balance(&sender, "uluna")
        .unwrap();

    assert_eq!(balance.amount, Uint128::zero());

    let ibc_denom = Ics20Helper::compute_ibc_denom_from_trace("transfer/channel-0/uluna");

    let balance = osmosis
        .borrow()
        .app
        .wrap()
        .query_balance(&contract_osmosis, ibc_denom)
        .unwrap();

    assert_eq!(balance.amount, amount.amount)
}

#[test]
fn ibc_hook_failing_execution() {
    let TestIbcHookEnv {
        eco,
        terra,
        osmosis,
        contract_osmosis,
        ..
    } = startup();

    let sender = terra.borrow().app.api().addr_make("sender");
    let receiver = osmosis.borrow().app.api().addr_make("receiver");

    let amount = Coin::new(1_000_000_u128, "uluna");

    let msg = CosmosMsg::Ibc(IbcMsg::Transfer {
        channel_id: "channel-0".to_string(),
        to_address: receiver.to_string(),
        amount: amount.clone(),
        timeout: IbcTimeout::with_timestamp(osmosis.borrow().app.block_info().time.plus_seconds(1)),
        memo: Some(
            serde_json::to_string_pretty(&MemoField::new(
                Some(WasmField {
                    contract: contract_osmosis.to_string(),
                    msg: counter::ExecuteMsg::JustReceive {
                        msg: "test".to_string(),
                        to_fail: true,
                    },
                }),
                None,
            ))
            .unwrap(),
        ),
    });

    terra
        .borrow_mut()
        .app
        .execute(sender.clone(), msg.clone())
        .unwrap_err();

    terra
        .borrow_mut()
        .app
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: sender.to_string(),
            amount: vec![amount.clone()],
        }))
        .unwrap();

    terra.borrow_mut().app.execute(sender.clone(), msg).unwrap();

    eco.relay_all_packets().unwrap();

    let balance = terra
        .borrow()
        .app
        .wrap()
        .query_balance(&sender, "uluna")
        .unwrap();

    assert_eq!(balance.amount, amount.amount);

    let ibc_denom = Ics20Helper::compute_ibc_denom_from_trace("transfer/channel-0/uluna");

    let balance = osmosis
        .borrow()
        .app
        .wrap()
        .query_balance(&contract_osmosis, ibc_denom)
        .unwrap();

    assert_eq!(balance.amount, Uint128::zero())
}

#[test]
fn ibc_hook_empty_memo() {
    let TestIbcHookEnv {
        eco,
        terra,
        osmosis,
        ..
    } = startup();

    let sender = terra.borrow().app.api().addr_make("sender");
    let receiver = osmosis.borrow().app.api().addr_make("receiver");

    let amount = Coin::new(1_000_000_u128, "uluna");

    let msg = CosmosMsg::Ibc(IbcMsg::Transfer {
        channel_id: "channel-0".to_string(),
        to_address: receiver.to_string(),
        amount: amount.clone(),
        timeout: IbcTimeout::with_timestamp(osmosis.borrow().app.block_info().time.plus_seconds(1)),
        memo: None,
    });

    terra
        .borrow_mut()
        .app
        .execute(sender.clone(), msg.clone())
        .unwrap_err();

    terra
        .borrow_mut()
        .app
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: sender.to_string(),
            amount: vec![amount.clone()],
        }))
        .unwrap();

    terra.borrow_mut().app.execute(sender.clone(), msg).unwrap();

    let balance = terra
        .borrow()
        .app
        .wrap()
        .query_balance(&sender, "uluna")
        .unwrap();

    assert_eq!(balance.amount, Uint128::zero());

    eco.relay_all_packets().unwrap();

    let ibc_denom = Ics20Helper::compute_ibc_denom_from_trace("transfer/channel-0/uluna");

    let balance = osmosis
        .borrow()
        .app
        .wrap()
        .query_balance(&receiver, ibc_denom)
        .unwrap();

    assert_eq!(balance.amount, amount.amount)
}

#[test]
fn ibc_hook_with_ibc_callback_ok() {
    let TestIbcHookEnv {
        eco,
        terra,
        osmosis,
        contract_osmosis,
        contract_terra,
        ..
    } = startup();

    let sender = terra.borrow().app.api().addr_make("sender");
    let receiver = osmosis.borrow().app.api().addr_make("receiver");

    let amount = Coin::new(1_000_000_u128, "uluna");

    let msg = CosmosMsg::Ibc(IbcMsg::Transfer {
        channel_id: "channel-0".to_string(),
        to_address: receiver.to_string(),
        amount: amount.clone(),
        timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(
            osmosis.borrow().app.block_info().time.seconds() + 1,
        )),
        memo: Some(
            serde_json::to_string_pretty(&MemoField::new(
                Some(WasmField {
                    contract: contract_osmosis.to_string(),
                    msg: counter::ExecuteMsg::JustReceive {
                        msg: "test".to_string(),
                        to_fail: false,
                    },
                }),
                Some(contract_terra.to_string()),
            ))
            .unwrap(),
        ),
    });

    terra
        .borrow_mut()
        .app
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: sender.to_string(),
            amount: vec![amount.clone()],
        }))
        .unwrap();

    terra.borrow_mut().app.execute(sender.clone(), msg).unwrap();

    eco.relay_all_packets().unwrap();

    let balance = terra
        .borrow()
        .app
        .wrap()
        .query_balance(&sender, "uluna")
        .unwrap();

    assert_eq!(balance.amount, Uint128::zero());

    let ibc_denom = Ics20Helper::compute_ibc_denom_from_trace("transfer/channel-0/uluna");

    let balance = osmosis
        .borrow()
        .app
        .wrap()
        .query_balance(&contract_osmosis, ibc_denom)
        .unwrap();

    assert_eq!(balance.amount, amount.amount);

    let ibc_counter: CounterConfig = terra
        .borrow()
        .app
        .wrap()
        .query_wasm_smart(contract_terra, &CounterQueryMsg::Config)
        .unwrap();

    assert_eq!(ibc_counter.counter_ibc_callback, 1);
}

#[test]
fn ibc_hook_with_ibc_callback_failing() {
    let TestIbcHookEnv {
        eco,
        terra,
        osmosis,
        contract_osmosis,
        contract_terra,
        ..
    } = startup();

    let sender = terra.borrow().app.api().addr_make("sender");
    let receiver = osmosis.borrow().app.api().addr_make("receiver");

    let amount = Coin::new(1_000_000_u128, "uluna");

    let msg = CosmosMsg::Ibc(IbcMsg::Transfer {
        channel_id: "channel-0".to_string(),
        to_address: receiver.to_string(),
        amount: amount.clone(),
        timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(
            osmosis.borrow().app.block_info().time.seconds() - 1,
        )),
        memo: Some(
            serde_json::to_string_pretty(&MemoField::new(
                Some(WasmField {
                    contract: contract_osmosis.to_string(),
                    msg: counter::ExecuteMsg::JustReceive {
                        msg: "test".to_string(),
                        to_fail: false,
                    },
                }),
                Some(contract_terra.to_string()),
            ))
            .unwrap(),
        ),
    });

    terra
        .borrow_mut()
        .app
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: sender.to_string(),
            amount: vec![amount.clone()],
        }))
        .unwrap();

    terra.borrow_mut().app.execute(sender.clone(), msg).unwrap();

    eco.relay_all_packets().unwrap();

    let ibc_counter: CounterConfig = terra
        .borrow()
        .app
        .wrap()
        .query_wasm_smart(contract_terra, &CounterQueryMsg::Config)
        .unwrap();

    // For testing purposes, if the packet is timedout, after increasing the counter_ibc_callback the contract raises an error.
    // This should revert the state of the contract but not the whole transaction,

    assert_eq!(ibc_counter.counter_ibc_callback, 0);

    let balance = terra
        .borrow()
        .app
        .wrap()
        .query_balance(&sender, "uluna")
        .unwrap();

    // Even if the ibc_callbacks failed the exectution, the funds transfer should be reverted.
    assert_eq!(balance.amount, balance.amount);

    let ibc_denom = Ics20Helper::compute_ibc_denom_from_trace("transfer/channel-0/uluna");

    let balance = osmosis
        .borrow()
        .app
        .wrap()
        .query_balance(&contract_osmosis, ibc_denom)
        .unwrap();

    assert_eq!(balance.amount, Uint128::zero());
}
