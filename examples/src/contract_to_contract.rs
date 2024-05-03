use cosmwasm_std::{to_json_binary, IbcMsg, IbcOrder, IbcTimeout, Timestamp};
use cw_iper_test::{
    cw_multi_test::{no_init, AppBuilder, ContractWrapper, Executor, MockApiBech32},
    AppExt, ContractWrapperExt, Ecosystem, IbcChannelCreator, IbcClosures, IbcPort, IperAppBuilder,
    IperIbcModule, IperStargateModule, IperContract,
};

use crate::mock_contracts::counter::{self, CounterConfig, CounterPacketData, CounterQueryMsg};

#[test]
fn contract_to_contract() {
    let neutron = AppBuilder::new()
        .with_api(MockApiBech32::new("neutron"))
        .with_ibc(IperIbcModule::default())
        .with_stargate(IperStargateModule::default())
        .build(no_init)
        .into_iper_app("neutron");

    let osmosis = IperAppBuilder::new("osmo")
        .build(no_init)
        .into_iper_app("osmosis");

    let eco = Ecosystem::default()
        .add_app(neutron.clone())
        .add_app(osmosis.clone());

    let contract = IperContract::new(
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

    let code_id_neutron = neutron.borrow_mut().store_ibc_code(contract);

    let contract = IperContract::new(
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

    let neutron_owner = neutron.borrow().app.api().addr_make("owner");
    let osmosis_owner = osmosis.borrow().app.api().addr_make("owner");

    let neutron_addr = neutron
        .borrow_mut()
        .app
        .instantiate_contract(
            code_id_neutron,
            neutron_owner.clone(),
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
            IbcPort::Contract(neutron_addr.clone()),
            IbcOrder::Unordered,
            "version",
            "connection_id",
            "neutron",
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
        data: to_json_binary(&CounterPacketData::Ok).unwrap(),
        timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(
            osmosis.borrow().app.block_info().time.seconds() + 1,
        )),
    };

    neutron
        .borrow_mut()
        .app
        .execute_contract(
            neutron_owner,
            neutron_addr.clone(),
            &counter::ExecuteMsg::SendPacket(msg),
            &[],
        )
        .unwrap();

    eco.relay_all_packets().unwrap();

    let counter_src_ack_ok = neutron
        .borrow()
        .app
        .wrap()
        .query_wasm_smart::<CounterConfig>(&neutron_addr, &CounterQueryMsg::Config)
        .unwrap()
        .counter_packet_ack_ok;

    assert_eq!(counter_src_ack_ok, 1);

    let counter_receive_dest = osmosis
        .borrow()
        .app
        .wrap()
        .query_wasm_smart::<CounterConfig>(&osmosis_addr, &CounterQueryMsg::Config)
        .unwrap()
        .counter_packet_receive;

    assert_eq!(counter_receive_dest, 1);
}
