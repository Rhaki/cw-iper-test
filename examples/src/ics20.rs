use cosmwasm_std::{
    AnyMsg, BankQuery, Coin, CosmosMsg, IbcMsg, IbcOrder, IbcTimeout, QueryRequest, SupplyResponse,
    Uint128,
};
use cw_iper_test::cw_multi_test::{
    no_init, AppBuilder, BankSudo, Executor, MockApiBech32, SudoMsg,
};
use cw_iper_test::ibc_applications::Ics20;
use cw_iper_test::ibc_applications::Ics20Helper;

use cw_iper_test::{
    AppBuilderIperExt, AppExt, Ecosystem, IbcChannelCreator, IbcPort, IperAppBuilder,
    IperIbcModule, IperStargateModule,
};
use ibc_proto::cosmos::base::v1beta1::Coin as IbcCoin;
use ibc_proto::ibc::apps::transfer::v1::MsgTransfer;

use prost::Message;

#[test]
fn base_ics20_transfer() {
    let neutron = AppBuilder::new()
        .with_api(MockApiBech32::new("neutron"))
        .with_ibc(IperIbcModule::default())
        .with_stargate(IperStargateModule::default())
        .with_ibc_app(Ics20)
        .build(no_init)
        .into_iper_app("neutron");

    let osmosis = IperAppBuilder::new("osmo")
        .with_ibc_app(Ics20)
        .build(no_init)
        .into_iper_app("osmosis");

    let eco = Ecosystem::default()
        .add_app(neutron.clone())
        .add_app(osmosis.clone());

    eco.open_ibc_channel(
        IbcChannelCreator::new(
            IbcPort::from_application(Ics20),
            IbcOrder::Unordered,
            "version",
            "connection_id",
            "neutron",
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

    let sender = neutron.borrow().app.api().addr_make("sender");
    let receiver = osmosis.borrow().app.api().addr_make("receiver");

    let amount = Coin::new(1_000_000_u128, "untrn");

    let msg = CosmosMsg::Ibc(IbcMsg::Transfer {
        channel_id: "channel-0".to_string(),
        to_address: receiver.to_string(),
        amount: amount.clone(),
        timeout: IbcTimeout::with_timestamp(osmosis.borrow().app.block_info().time.plus_seconds(1)),
        memo: None,
    });

    neutron
        .borrow_mut()
        .app
        .execute(sender.clone(), msg.clone())
        .unwrap_err();

    neutron
        .borrow_mut()
        .app
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: sender.to_string(),
            amount: vec![amount.clone()],
        }))
        .unwrap();

    neutron.borrow_mut().app.execute(sender.clone(), msg).unwrap();

    eco.relay_all_packets().unwrap();

    let balance = neutron
        .borrow()
        .app
        .wrap()
        .query_balance(&sender, "untrn")
        .unwrap();

    assert_eq!(balance.amount, Uint128::zero());

    let supply = neutron
        .borrow()
        .app
        .wrap()
        .query::<SupplyResponse>(&QueryRequest::Bank(BankQuery::Supply {
            denom: "untrn".to_string(),
        }))
        .unwrap();

    assert_eq!(supply.amount.amount, amount.amount);

    let ibc_denom = Ics20Helper::compute_ibc_denom_from_trace("transfer/channel-0/untrn");

    let balance = osmosis
        .borrow()
        .app
        .wrap()
        .query_balance(&receiver, &ibc_denom)
        .unwrap();

    assert_eq!(balance.amount, amount.amount);

    // Send tokens back

    let msg = CosmosMsg::Ibc(IbcMsg::Transfer {
        channel_id: "channel-0".to_string(),
        to_address: sender.to_string(),
        amount: Coin::new(amount.amount, &ibc_denom),
        timeout: IbcTimeout::with_timestamp(neutron.borrow().app.block_info().time.plus_seconds(1)),
        memo: None,
    });

    osmosis
        .borrow_mut()
        .app
        .execute(receiver.clone(), msg)
        .unwrap();

    eco.relay_all_packets().unwrap();

    let balance = neutron
        .borrow()
        .app
        .wrap()
        .query_balance(&sender, "untrn")
        .unwrap();

    assert_eq!(balance.amount, amount.amount);

    let balance = osmosis
        .borrow()
        .app
        .wrap()
        .query_balance(&receiver, &ibc_denom)
        .unwrap();

    assert_eq!(balance.amount, Uint128::zero());

    let supply = osmosis
        .borrow()
        .app
        .wrap()
        .query::<SupplyResponse>(&QueryRequest::Bank(BankQuery::Supply {
            denom: ibc_denom.to_string(),
        }))
        .unwrap();

    assert_eq!(supply.amount.amount, Uint128::zero());
}

#[test]
fn stargate_ics20_transfer() {
    let neutron = AppBuilder::new()
        .with_api(MockApiBech32::new("neutron"))
        .with_ibc(IperIbcModule::default())
        .with_stargate(IperStargateModule::default())
        .with_ibc_app(Ics20)
        .build(no_init)
        .into_iper_app("neutron");

    let osmosis = IperAppBuilder::new("osmo")
        .with_ibc_app(Ics20)
        .build(no_init)
        .into_iper_app("osmosis");

    let eco = Ecosystem::default()
        .add_app(neutron.clone())
        .add_app(osmosis.clone());

    eco.open_ibc_channel(
        IbcChannelCreator::new(
            IbcPort::from_application(Ics20),
            IbcOrder::Unordered,
            "version",
            "connection_id",
            "neutron",
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

    let sender = neutron.borrow().app.api().addr_make("sender");
    let receiver = osmosis.borrow().app.api().addr_make("receiver");

    let amount = Coin::new(1_000_000_u128, "untrn");

    let msg = MsgTransfer {
        source_port: "transfer".to_string(),
        source_channel: "channel-0".to_string(),
        token: Some(IbcCoin {
            denom: amount.denom.clone(),
            amount: amount.amount.to_string(),
        }),
        sender: sender.to_string(),
        receiver: receiver.to_string(),
        timeout_height: None,
        timeout_timestamp: osmosis.borrow().app.block_info().time.nanos() + 1,
        memo: "".to_string(),
    };

    #[allow(deprecated)]
    let msg = CosmosMsg::Any(AnyMsg {
        type_url: "/ibc.applications.transfer.v1.MsgTransfer".to_string(),
        value: msg.encode_to_vec().into(),
    });

    neutron
        .borrow_mut()
        .app
        .execute(sender.clone(), msg.clone())
        .unwrap_err();

    neutron
        .borrow_mut()
        .app
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: sender.to_string(),
            amount: vec![amount.clone()],
        }))
        .unwrap();

    neutron.borrow_mut().app.execute(sender.clone(), msg).unwrap();

    eco.relay_all_packets().unwrap();

    let balance = neutron
        .borrow()
        .app
        .wrap()
        .query_balance(&sender, "untrn")
        .unwrap();

    assert_eq!(balance.amount, Uint128::zero());

    let ibc_denom = Ics20Helper::compute_ibc_denom_from_trace("transfer/channel-0/untrn");

    let balance = osmosis
        .borrow()
        .app
        .wrap()
        .query_balance(&receiver, ibc_denom)
        .unwrap();

    assert_eq!(balance.amount, amount.amount)
}

#[test]
fn failing_ics20_transfer() {
    let neutron = AppBuilder::new()
        .with_api(MockApiBech32::new("neutron"))
        .with_ibc(IperIbcModule::default())
        .with_stargate(IperStargateModule::default())
        .with_ibc_app(Ics20)
        .build(no_init)
        .into_iper_app("neutron");

    let osmosis = IperAppBuilder::new("osmo")
        .with_ibc_app(Ics20)
        .build(no_init)
        .into_iper_app("osmosis");

    let eco = Ecosystem::default()
        .add_app(neutron.clone())
        .add_app(osmosis.clone());

    eco.open_ibc_channel(
        IbcChannelCreator::new(
            IbcPort::from_application(Ics20),
            IbcOrder::Unordered,
            "version",
            "connection_id",
            "neutron",
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

    let sender = neutron.borrow().app.api().addr_make("sender");
    let receiver = osmosis.borrow().app.api().addr_make("receiver");

    let amount = Coin::new(1_000_000_u128, "untrn");

    let msg = CosmosMsg::Ibc(IbcMsg::Transfer {
        channel_id: "channel-0".to_string(),
        to_address: receiver.to_string(),
        amount: amount.clone(),
        timeout: IbcTimeout::with_timestamp(
            osmosis.borrow().app.block_info().time.minus_seconds(1),
        ),
        memo: None,
    });

    neutron
        .borrow_mut()
        .app
        .execute(sender.clone(), msg.clone())
        .unwrap_err();

    neutron
        .borrow_mut()
        .app
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: sender.to_string(),
            amount: vec![amount.clone()],
        }))
        .unwrap();

    neutron.borrow_mut().app.execute(sender.clone(), msg).unwrap();

    eco.relay_all_packets().unwrap();

    let balance = neutron
        .borrow()
        .app
        .wrap()
        .query_balance(&sender, "untrn")
        .unwrap();

    assert_eq!(balance.amount, amount.amount);

    let ibc_denom = Ics20Helper::compute_ibc_denom_from_trace("transfer/channel-0/untrn");

    let balance = osmosis
        .borrow()
        .app
        .wrap()
        .query_balance(&receiver, ibc_denom)
        .unwrap_or_default();

    assert_eq!(balance.amount, Uint128::zero())
}

#[test]
fn timeout_ics20_transfer() {
    let neutron = AppBuilder::new()
        .with_api(MockApiBech32::new("neutron"))
        .with_ibc(IperIbcModule::default())
        .with_stargate(IperStargateModule::default())
        .with_ibc_app(Ics20)
        .build(no_init)
        .into_iper_app("neutron");

    let osmosis = IperAppBuilder::new("osmo")
        .with_ibc_app(Ics20)
        .build(no_init)
        .into_iper_app("osmosis");

    let eco = Ecosystem::default()
        .add_app(neutron.clone())
        .add_app(osmosis.clone());

    eco.open_ibc_channel(
        IbcChannelCreator::new(
            IbcPort::from_application(Ics20),
            IbcOrder::Unordered,
            "version",
            "connection_id",
            "neutron",
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

    let sender = neutron.borrow().app.api().addr_make("sender");
    let receiver = "invalid_address".to_string();

    let amount = Coin::new(1_000_000_u128, "untrn");

    let msg = CosmosMsg::Ibc(IbcMsg::Transfer {
        channel_id: "channel-0".to_string(),
        to_address: receiver.to_string(),
        amount: amount.clone(),
        timeout: IbcTimeout::with_timestamp(osmosis.borrow().app.block_info().time.plus_seconds(1)),
        memo: None,
    });

    neutron
        .borrow_mut()
        .app
        .execute(sender.clone(), msg.clone())
        .unwrap_err();

    neutron
        .borrow_mut()
        .app
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: sender.to_string(),
            amount: vec![amount.clone()],
        }))
        .unwrap();

    neutron.borrow_mut().app.execute(sender.clone(), msg).unwrap();

    eco.relay_all_packets().unwrap();

    let balance = neutron
        .borrow()
        .app
        .wrap()
        .query_balance(&sender, "untrn")
        .unwrap();

    assert_eq!(balance.amount, amount.amount);

    let ibc_denom = Ics20Helper::compute_ibc_denom_from_trace("transfer/channel-0/untrn");

    let balance = osmosis
        .borrow()
        .app
        .wrap()
        .query_balance(&receiver, ibc_denom)
        .unwrap_or_default();

    assert_eq!(balance.amount, Uint128::zero())
}
