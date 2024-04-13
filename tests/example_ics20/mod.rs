use cosmwasm_std::IbcOrder;
use cw_iper_test::{
    app_ext::AppExt as _,
    ecosystem::Ecosystem,
    ibc::{IbcChannelCreator, IbcPort},
    ibc_app_builder::{AppBuilderExt, IbcAppBuilder},
    ibc_applications::Ics20,
    ibc_module::IbcModule,
};
use cw_multi_test::{no_init, AppBuilder, MockApiBech32};

#[test]
fn base() {
    let terra = AppBuilder::new()
        .with_api(MockApiBech32::new("terra"))
        .with_ibc(IbcModule::default())
        .with_ibc_app(Ics20::default())
        .build(no_init)
        .into_ibc_app("terra");

    let osmosis = IbcAppBuilder::new("osmo")
        .with_ibc_app(Ics20::default())
        .build(no_init)
        .into_ibc_app("osmosis");

    let eco = Ecosystem::default()
        .add_app(terra.clone())
        .add_app(osmosis.clone());

    eco.open_ibc_channel(
        IbcChannelCreator::new(
            IbcPort::Module(Ics20::NAME.to_string()),
            IbcOrder::Unordered,
            "version",
            "connection_id",
            "terra",
        ),
        IbcChannelCreator::new(
            IbcPort::Module(Ics20::NAME.to_string()),
            IbcOrder::Unordered,
            "version",
            "connection_id",
            "osmosis",
        ),
    )
    .unwrap();
}
