use cosmwasm_std::{Coin, CosmosMsg, IbcMsg, IbcOrder, IbcTimeout, Timestamp, Uint128};
use cw_iper_test::{
    app_ext::AppExt,
    cw_multi_test::{no_init, BankSudo, Executor, SudoMsg},
    ecosystem::Ecosystem,
    ibc::{IbcChannelCreator, IbcPort},
    ibc_app_builder::{AppBuilderIbcExt, IbcAppBuilder},
    ibc_applications::{IbcHook, Ics20},
};

#[test]
fn base_ics20_transfer() {
    let osmosis = IbcAppBuilder::new("osmo")
        .with_ibc_app(Ics20::default())
        .build(no_init)
        .into_ibc_app("osmosis");

    let terra = IbcAppBuilder::new("terra")
        .with_ibc_app(IbcHook::new(Ics20::default()))
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
        timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(123)),
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

    eco.relay_all_packets().unwrap();

    let balance = terra
        .borrow()
        .app
        .wrap()
        .query_balance(&sender, "uluna")
        .unwrap();

    assert_eq!(balance.amount, Uint128::zero());

    let balance = osmosis
        .borrow()
        .app
        .wrap()
        .query_balance(&receiver, "mock_denom")
        .unwrap();

    assert_eq!(balance.amount, amount.amount)
}
