use cosmwasm_std::{to_json_binary, Addr, IbcMsg, IbcOrder, IbcTimeout, Timestamp};
use cw_iper_test::{
    app_ext::AppExt,
    contracts::{ContractWrapperExt, IbcClosures, MultiContract},
    ibc_app_builder::IbcAppBuilder,
    ibc_module::{IbcChannelCreator, IbcModule, IbcPort},
    iper_app::IperApp,
};
use cw_multi_test::{no_init, AppBuilder, ContractWrapper, Executor};

use crate::contracts::mock_1;

#[test]
#[rustfmt::skip]
fn t1() {
    let app_1 = AppBuilder::new()
        .with_ibc(IbcModule::default())
        .build(no_init)
        .into_ibc_app();

    let app_2 = IbcAppBuilder::new()
        .build(no_init)
        .into_ibc_app();

    let mut app = IperApp::new(app_1, app_2);

    let contract = MultiContract::new(
        ContractWrapper::new(mock_1::execute, mock_1::instantiate, mock_1::query).as_contract(),
        Some(IbcClosures::new_as_ibc_contract(
            mock_1::ibc_channel_open,
            mock_1::ibc_channel_close,
            mock_1::ibc_channel_connect,
            mock_1::ibc_packet_receive,
            mock_1::ibc_packet_ack,
            mock_1::ibc_packet_timeout,
        )),
    );

    let code_id_1 = app.store_code_on_1(contract);

    let contract = MultiContract::new(
        ContractWrapper::new(mock_1::execute, mock_1::instantiate, mock_1::query).as_contract(),
        Some(IbcClosures::new_as_ibc_contract(
            mock_1::ibc_channel_open,
            mock_1::ibc_channel_close,
            mock_1::ibc_channel_connect,
            mock_1::ibc_packet_receive,
            mock_1::ibc_packet_ack,
            mock_1::ibc_packet_timeout,
        )),
    );
    let code_id_2 = app.store_code_on_2(contract);

    let addr_1 = app.app_1_mut()
        .instantiate_contract(
            code_id_1,
            Addr::unchecked("sender"),
            &mock_1::InstantiateMsg {},
            &[],
            "label".to_string(),
            None,
        )
        .unwrap();

    let addr_2 = app
        .app_2_mut()
        .instantiate_contract(
            code_id_2,
            Addr::unchecked("sender"),
            &mock_1::InstantiateMsg {},
            &[],
            "label".to_string(),
            None,
        )
        .unwrap();

    app.open_ibc_channel(
        IbcChannelCreator::new(IbcPort::Contract(addr_1.clone()), IbcOrder::Unordered, "version", "connection_id"),
        IbcChannelCreator::new(IbcPort::Contract(addr_2.clone()), IbcOrder::Unordered, "version", "connection_id")
    )
    .unwrap();


    let msg = IbcMsg::SendPacket {
        channel_id: "channel-0".to_string(),
        data: to_json_binary("some_ack").unwrap(),
        timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(10)),
    };

    app.app_1_mut()
        .execute_contract(
            Addr::unchecked("sender"),
            addr_1,
            &mock_1::ExecuteMsg::SendPacket(msg),
            &[],
        )
        .unwrap();


    app.relay_all_packets(Addr::unchecked("relayer")).unwrap();

    let pending_packets = app.pending_packets();

    println!("{pending_packets:#?}")
}
