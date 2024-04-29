use cosmwasm_std::{Coin, CosmosMsg, IbcMsg, IbcOrder, IbcTimeout, Timestamp, Uint128};
use cw_iper_test::{
    app_ext::AppExt,
    contracts::{ContractWrapperExt, MultiContract},
    cw_multi_test::{no_init, BankSudo, ContractWrapper, Executor, SudoMsg},
    ecosystem::Ecosystem,
    ibc::{IbcChannelCreator, IbcPort},
    ibc_app_builder::{AppBuilderIbcExt, IbcAppBuilder},
    ibc_applications::{IbcHook, Ics20, Ics20Helper, MemoField, WasmField},
};

use crate::mock_contracts::counter;

#[test]
fn ibc_hook_base() {
    let osmosis = IbcAppBuilder::new("osmo")
        .with_ibc_app(IbcHook::new(Ics20))
        .build(no_init)
        .into_ibc_app("osmosis");

    let terra = IbcAppBuilder::new("terra")
        .with_ibc_app(IbcHook::new(Ics20))
        .build(no_init)
        .into_ibc_app("terra");

    let eco = Ecosystem::default()
        .add_app(terra.clone())
        .add_app(osmosis.clone());

    let contract = MultiContract::new(
        ContractWrapper::new(counter::execute, counter::instantiate, counter::query).to_contract(),
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
            serde_json::to_string_pretty(&MemoField {
                wasm: Some(WasmField {
                    contract: osmosis_contract_addr.to_string(),
                    msg: counter::ExecuteMsg::JustReceive {
                        msg: "test".to_string(),
                        to_fail: false,
                    },
                }),
            })
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
        .query_balance(&osmosis_contract_addr, ibc_denom)
        .unwrap();

    assert_eq!(balance.amount, amount.amount)
}

#[test]
fn ibc_hook_failing_execution() {
    let osmosis = IbcAppBuilder::new("osmo")
        .with_ibc_app(IbcHook::new(Ics20))
        .build(no_init)
        .into_ibc_app("osmosis");

    let terra = IbcAppBuilder::new("terra")
        .with_ibc_app(IbcHook::new(Ics20))
        .build(no_init)
        .into_ibc_app("terra");

    let eco = Ecosystem::default()
        .add_app(terra.clone())
        .add_app(osmosis.clone());

    let contract = MultiContract::new(
        ContractWrapper::new(counter::execute, counter::instantiate, counter::query).to_contract(),
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

    let sender = terra.borrow().app.api().addr_make("sender");
    let receiver = osmosis.borrow().app.api().addr_make("receiver");

    let amount = Coin::new(1_000_000_u128, "uluna");

    let msg = CosmosMsg::Ibc(IbcMsg::Transfer {
        channel_id: "channel-0".to_string(),
        to_address: receiver.to_string(),
        amount: amount.clone(),
        timeout: IbcTimeout::with_timestamp(osmosis.borrow().app.block_info().time.plus_seconds(1)),
        memo: Some(
            serde_json::to_string_pretty(&MemoField {
                wasm: Some(WasmField {
                    contract: osmosis_contract_addr.to_string(),
                    msg: counter::ExecuteMsg::JustReceive {
                        msg: "test".to_string(),
                        to_fail: true,
                    },
                }),
            })
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
        .query_balance(&osmosis_contract_addr, ibc_denom)
        .unwrap();

    assert_eq!(balance.amount, Uint128::zero())
}

#[test]
fn ibc_hook_empty_memo() {
    let osmosis = IbcAppBuilder::new("osmo")
        .with_ibc_app(IbcHook::new(Ics20))
        .build(no_init)
        .into_ibc_app("osmosis");

    let terra = IbcAppBuilder::new("terra")
        .with_ibc_app(IbcHook::new(Ics20))
        .build(no_init)
        .into_ibc_app("terra");

    let eco = Ecosystem::default()
        .add_app(terra.clone())
        .add_app(osmosis.clone());

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
