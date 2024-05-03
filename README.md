# CosmWasm IperTest

[![cw-iper-test on crates.io][crates-badge]][crates-url]

[crates-badge]: https://img.shields.io/crates/v/cw-iper-test.svg
[crates-url]: https://crates.io/crates/cw-iper-test

**Testing tools for <u>_ibc_</u> multi-contract interactions**

## Introduction

`cw-iper-test` is a testing solution built on top of [**cw-multi-test**](https://github.com/CosmWasm/cw-multi-test) that enables `smart contract` testing in an` IBC-enabled` environment.

This framework allows testing for:

- Contracts that implement IBC `entry-points`;
- IBC applications that interact with `smart contract`s (`IbcHook`);
- Complete simulation of a packet exchange between two blockchains (represented by the `App` structure of `cw-multi-test`).

> **_DISCLAIMER:_**
>
> - The library is in a testing version and should be used with caution. Any feedback, bug reports, or contributions are welcome.
> - Currently, the library depends on a [**forked**](https://github.com/Rhaki/rhaki-cw-multi-test) version of `cw-multi-test` as some minor modifications are necessary. Once the code is stabilized, the necessary changes will be proposed through a PR.

## How It Works

`cw-iper-test` introduces a series of structures and interfaces that extend the existing classes of `cw-multi-test`. Specifically:

- **[IperApp](./cw-iper-test/src/iper_app.rs#L58)**: A structure that wraps the `App` structure of `cw-multi-test`. It extends functionalities related to the handling of incoming and outgoing IBC packets, as well as the ability to store contracts with IBC `entry-points`. The structure exposes the `App` class to utilize various internal methods (`execute_contract`, `wrap`, etc.).

- **[Ecosystem](./cw-iper-test/src/ecosystem.rs#L18)**: A structure that groups various `IperApp` instances. It is responsible for opening IBC channels and relaying packets.

- **[IperStargateModule](./cw-iper-test/src/stargate.rs#L32)** & **[IperIbcModule](./cw-iper-test/src/ibc_module.rs#L45)**: These are custom versions of the Stargate and IBC modules used by the `App` of `cw-multi-test`. An `IperApp` requires that the internal `App` uses these two modules. Specifically, they allow for:

  - **IperStargateModule**: Contains a collection of **[StargateApplication](./cw-iper-test/src/stargate.rs#L187)**. `StargateApplication` is a trait that defines a module accepting Stargate messages and queries (for example, `TokenFactory`, or even IBC modules, such as `Ics20`). During the creation of an `IperApp`, a list of structures implementing these traits can be added.
  - **IperIbcModule**: Similar to `IperStargateModule`, it contains a collection of **[IbcApplication](./cw-iper-test/src/ibc_application.rs#L45)**. When an `IbcMsg` needs to be handled, it checks if there is an `IbcApplication` that defines the source channel's port and allows the application to perform actions. Moreover, when `IperApp` receives a `packet` (or an `acknowledgment`, `timeout`), it tries to load the target `IbcApplication` and asks it to handle the `packet`.

- **[Middleware](./cw-iper-test/src/middleware.rs#L64)**: `Middleware` is a trait that by default implements both `IbcApplication` and `StargateApplication`. It allows for wrapping an `IbcApplication` to enhance its functionality (see **[IbcHook](./cw-iper-test/src/ibc_applications/ibc_hook.rs#L86)** as an example). The core concept is that when an `incoming` or `outgoing` `packet` needs to be handled and the IBC channel's `port` is the `wrapped application`, the `Middleware` is triggered on two functions: one `before` (before calling the function of the wrapped IBC application) and one `after` (after its execution).
  It is recommended to examine the `IbcHook` and to read the comments in the trait definition for better integration understanding.

## Examples

<details>
    <summary><strong>Packet between Contract <-> Contract </summary>

```rust
imports
#[test]
fn contract_to_contract() {
    // Create new IperApp.
    // Is possible to user default AppBuilder from cw-multi-test
    // adding api, ibc and stargate modules as following
    let neutron = AppBuilder::new()
        .with_api(MockApiBech32::new("neutron"))
        .with_ibc(IperIbcModule::default())
        .with_stargate(IperStargateModule::default())
        .build(no_init)
        .into_iper_app("neutron"); // transform App into IperApp

    // Or use IperAppBuilder
    let osmosis = IperAppBuilder::new("osmo")
        .build(no_init)
        .into_iper_app("osmosis");

    // Create an Ecosystem wih both neutron and osmosis app
    let eco = Ecosystem::default()
        .add_app(neutron.clone())
        .add_app(osmosis.clone());

    // Create a IperContract with
    // cw-multi-test ContractWrapper for default entry-points
    // IbcClosures for ibc entry-points
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

    // Store code id using store_ibc_code functions of IperApp
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

    // generate address for init contract
    let neutron_owner = neutron.borrow().app.api().addr_make("owner");
    let osmosis_owner = osmosis.borrow().app.api().addr_make("owner");

    // instantiate contracts using instantiate_contract from inner App of IperApp
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

    // Open a ibc channel using Ecosystem, specifing as port the two address instantiated.
    eco.open_ibc_channel(
        IbcChannelCreator::new(
            IbcPort::Contract(neutron_addr.clone()),
            IbcOrder::Unordered, // currently order has no impapact beside contract internal usage
            "version", // currently version has no impact beside contract internal usage
            "connection_id", // currently connection id has no impact contract internal usage
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
        // CounterPacketData::Ok means that on destination chain, the ack will be Ok
        data: to_json_binary(&CounterPacketData::Ok).unwrap(),
        timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(
            osmosis.borrow().app.block_info().time.seconds() + 1,
        )),
    };

    // Execute the contract using the ExecuteMsg variant SendPacket.
    // This testing contract basically append into the response the IbcMsg.
    // This will trigger the IbcModule, but since the source port is a contract,
    // only a packet will be emitted.
    neutron
        .borrow_mut()
        .app
        .execute_contract(
            neutron_owner,
            neutron_addr,
            &counter::ExecuteMsg::SendPacket(msg),
            &[],
        )
        .unwrap();

    // Is now possile relay the packet.
    // Using relay_all_packets from Ecosystem, all packets will be relayed.
    // When the first packet arrive on destination chain, the packet receive will be triggered.
    // If an ack packet will be emitted, the eco will relay it until any chains has no pending packet.
    eco.relay_all_packets().unwrap();

    // Query the contract Config, check if the counter_receive_dest has been increased
    // on destination chain
    let counter_receive_dest = osmosis
        .borrow()
        .app
        .wrap()
        .query_wasm_smart::<CounterConfig>(&osmosis_addr, &CounterQueryMsg::Config)
        .unwrap()
        .counter_packet_receive;

    assert_eq!(counter_receive_dest, 1);

    // Query the contract Config, check if the counter_src_ack_ok has been increased
    // on src chain
    let counter_src_ack_ok = neutron
        .borrow()
        .app
        .wrap()
        .query_wasm_smart::<CounterConfig>(&neutron_addr, &CounterQueryMsg::Config)
        .unwrap()
        .counter_packet_ack_ok;

    assert_eq!(counter_src_ack_ok, 1);
}

```

</details>

<details>
    <summary><strong>Ibc Hook </summary>

```rust
imports
#[test]
fn ibc_hook_base() {

    // Create new IperApp using IperAppBuilder
    let osmosis = IperAppBuilder::new("osmo")
        // Add IbcHook that wrap Ics20
        // This because IbcHook is a Middleware
        .with_ibc_app(IbcHook::new(Ics20))
        .build(no_init)
        .into_iper_app("osmosis");

    let neutron = IperAppBuilder::new("neutron")
        .with_ibc_app(IbcHook::new(Ics20))
        .build(no_init)
        .into_iper_app("neutron");

    // Create an Ecosystem wih both neutron and osmosis app
    let eco = Ecosystem::default()
        .add_app(neutron.clone())
        .add_app(osmosis.clone());

    // Create a IperContract with cw-multi-test ContractWrapper for default entry-points
    // IbcClosures are not needed because ibc hook doesn't require
    let contract = IperContract::new(
        ContractWrapper::new(counter::execute, counter::instantiate, counter::query)
            .with_sudo(counter::sudo)
            .to_contract(),
        None,
    );

    // Store code id using store_ibc_code functions of IperApp
    // In this case also osmosis.borrow_mut().app.store_code could be used
    let code_id_osmosis = osmosis.borrow_mut().store_ibc_code(contract);

    // generate address for init contract
    let osmosis_owner = osmosis.borrow().app.api().addr_make("owner");

    // instantiate contracts using instantiate_contract from inner App of IperApp
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

    // Open a ibc channel using Ecosystem, specifing as port the two Ics20 modules.
    // IbcHook is a middleware, it ports is equal to his children port
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

    // Create a sender
    let sender = neutron.borrow().app.api().addr_make("sender");
    // Create a receiver
    let receiver = osmosis.borrow().app.api().addr_make("receiver");

    let amount = Coin::new(1_000_000_u128, "untrn");

    // Mint the native coin to send
    neutron
        .borrow_mut()
        .app
        .sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: sender.to_string(),
            amount: vec![amount.clone()],
        }))
        .unwrap();

    // Create a IbcMsg::Transfer.
    // It could also possible to use StargateMsg or IbcMsg::SendPacket
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
                        // this filed if true make the contract Execution to fails
                        // at contract level
                        to_fail: false,
                    },
                }),
                // ibc_callback is not tested here
                None,
            ))
            .unwrap(),
        ),
    });

    // Execute the msg
    neutron
        .borrow_mut()
        .app
        .execute(sender.clone(), msg)
        .unwrap();


    // Is now possile relay the packet.
    // Using relay_all_packets from Ecosystem, all packets will be relayed.
    // When the first packet arrive on destination chain, the packet receive will be triggered.
    // If an ack packet will be emitted, the eco will relay it until any chains has no pending packet.
    eco.relay_all_packets().unwrap();

    // Balance on src chain has to be reduced
    let balance = neutron
        .borrow()
        .app
        .wrap()
        .query_balance(&sender, "untrn")
        .unwrap();

    assert_eq!(balance.amount, Uint128::zero());

    // Compute the ibc denom
    let ibc_denom = Ics20Helper::compute_ibc_denom_from_trace("transfer/channel-0/untrn");

    // Check if the contract has received the tokens
    let balance = osmosis
        .borrow()
        .app
        .wrap()
        .query_balance(&contract_osmosis, ibc_denom)
        .unwrap();

    assert_eq!(balance.amount, amount.amount);

    // Check also if the contract has been executed.
    // When ExecuteMsg::JustReceive is triggered,
    // the contract increase the counter_ibc_hook by 1
    let counter_ibc_hook = osmosis
        .borrow()
        .app
        .wrap()
        .query_wasm_smart::<CounterConfig>(&contract_osmosis, &CounterQueryMsg::Config)
        .unwrap()
        .counter_ibc_hook;

    assert_eq!(counter_ibc_hook, 1)
}

```

</details>
