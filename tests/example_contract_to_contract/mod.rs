use cosmwasm_std::{to_json_binary, IbcMsg, IbcOrder, IbcTimeout, Timestamp};
use cw_iper_test::{
    app_ext::AppExt,
    contracts::{ContractWrapperExt, IbcClosures, MultiContract},
    ecosystem::Ecosystem,
    ibc::{IbcChannelCreator, IbcPort},
    ibc_app_builder::IbcAppBuilder,
    ibc_module::IbcModule,
};
use cw_multi_test::Executor;
use cw_multi_test::{no_init, AppBuilder, ContractWrapper, MockApiBech32};

use crate::mock_contracts::counter;

#[test]
fn base() {
    let terra = AppBuilder::new()
        .with_api(MockApiBech32::new("terra"))
        .with_ibc(IbcModule::default())
        .build(no_init)
        .into_ibc_app("terra");

    let osmosis = IbcAppBuilder::new("osmo")
        .build(no_init)
        .into_ibc_app("osmosis");

    let eco = Ecosystem::default()
        .add_app(terra.clone())
        .add_app(osmosis.clone());

    let contract = MultiContract::new(
        ContractWrapper::new(counter::execute, counter::instantiate, counter::query).to_contract(),
        Some(IbcClosures::new_as_ibc_contract(
            counter::ibc_channel_open,
            counter::ibc_channel_close,
            counter::ibc_channel_connect,
            counter::ibc_packet_receive,
            counter::ibc_packet_ack,
            counter::ibc_packet_timeout,
        )),
    );

    let code_id_terra = terra.borrow_mut().store_ibc_code(contract);

    let contract = MultiContract::new(
        ContractWrapper::new(counter::execute, counter::instantiate, counter::query).to_contract(),
        Some(IbcClosures::new_as_ibc_contract(
            counter::ibc_channel_open,
            counter::ibc_channel_close,
            counter::ibc_channel_connect,
            counter::ibc_packet_receive,
            counter::ibc_packet_ack,
            counter::ibc_packet_timeout,
        )),
    );

    let code_id_osmosis = osmosis.borrow_mut().store_ibc_code(contract);

    let terra_owner = terra.borrow().app.api().addr_make("owner");
    let osmosis_owner = osmosis.borrow().app.api().addr_make("owner");

    let terra_addr = terra
        .borrow_mut()
        .app
        .instantiate_contract(
            code_id_terra,
            terra_owner.clone(),
            &counter::InstantiateMsg {},
            &[],
            "label".to_string(),
            None,
        )
        .unwrap();

    let osmosis_addr = osmosis
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
            IbcPort::Contract(terra_addr.clone()),
            IbcOrder::Unordered,
            "version",
            "connection_id",
            "terra",
        ),
        IbcChannelCreator::new(
            IbcPort::Contract(osmosis_addr.clone()),
            IbcOrder::Unordered,
            "version",
            "connection_id",
            "osmosis",
        ),
    )
    .unwrap();

    let msg = IbcMsg::SendPacket {
        channel_id: "channel-0".to_string(),
        data: to_json_binary("some_ack").unwrap(),
        timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(10)),
    };

    terra
        .borrow_mut()
        .app
        .execute_contract(
            terra_owner,
            terra_addr,
            &counter::ExecuteMsg::SendPacket(msg),
            &[],
        )
        .unwrap();

    eco.relay_all_packets().unwrap();

    // println!("{pending_packets:#?}")
}
