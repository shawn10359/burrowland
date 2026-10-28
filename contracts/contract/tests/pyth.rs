mod workspace_env;

use mock_boost_farming::nano_to_sec;

use crate::workspace_env::*;


#[tokio::test]
async fn test_liquidation_decrease_health_factor_with_pyth() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let pyth_contract = deploy_mock_pyth(&root).await?;
    let nusdc_token_contract = deploy_mock_ft(&root, "nusdc", 18).await?;
    let nusdt_token_contract = deploy_mock_ft(&root, "nusdt", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 18).await?;
    let wrap_reserve_amount = d(10000, 24);
    let nusdt_reserve_amount = d(10000, 6);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount));
    check!(nusdt_token_contract.ft_mint(&root, &root, nusdt_reserve_amount));

    let burrowland_contract = deploy_burrowland_with_pyth(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &nusdc_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &nusdt_token_contract));
    check!(nusdc_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(nusdt_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.deposit_to_reserve(&nusdt_token_contract, &root, nusdt_reserve_amount));
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, None));
    check!(burrowland_contract.add_token_pyth_info(&root, nusdt_token_contract.0.id(), 6, 4, "1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", None, None));
    check!(burrowland_contract.add_token_pyth_info(&root, nusdc_token_contract.0.id(), 6, 4, "41f3625971ca2ed2263e78573fe5ce23e13d2558ed3f2e47ab0f84fb9e7ae722", None, None));
    // usdt
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(pyth_contract.set_price("1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", PythPrice{
        price: I64(100000000),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    // usdc
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(pyth_contract.set_price("41f3625971ca2ed2263e78573fe5ce23e13d2558ed3f2e47ab0f84fb9e7ae722", PythPrice{
        price: I64(100000000),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    // near
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1000000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdc_token_contract.ft_mint(&root, &alice, supply_amount));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));
    check!(nusdt_token_contract.ft_storage_deposit(alice.id()));

    check!(burrowland_contract.supply_to_collateral(&nusdc_token_contract, &alice, (supply_amount / extra_decimals_mult).into()));

    let wnear_borrow_amount = d(50, 24);
    check!(burrowland_contract.borrow_and_withdraw_with_pyth(&alice, wrap_token_contract.0.id(), wnear_borrow_amount));

    let usdt_borrow_amount = d(50, 18);
    check!(burrowland_contract.borrow_and_withdraw_with_pyth(&alice, nusdt_token_contract.0.id(), usdt_borrow_amount));

    let account = burrowland_contract.get_account(&alice).await?.unwrap();
    assert!(account.supplied.is_empty());
    assert!(find_asset(&account.borrowed, wrap_token_contract.0.id()).apr > BigDecimal::zero());
    assert!(find_asset(&account.borrowed, nusdt_token_contract.0.id()).apr > BigDecimal::zero());

    let wnear_bobs_amount = d(100, 24);
    let usdt_bobs_amount = d(100, 18);
    let bob = create_account(&root, "bob", None).await;
    check!(burrowland_contract.storage_deposit(&bob));
    check!(nusdt_token_contract.ft_mint(&root, &bob, usdt_bobs_amount / extra_decimals_mult));
    check!(wrap_token_contract.ft_mint(&root, &bob, wnear_bobs_amount));
    check!(nusdc_token_contract.ft_storage_deposit(bob.id()));
    check!(burrowland_contract.deposit(&wrap_token_contract, &bob, wnear_bobs_amount));
    check!(burrowland_contract.deposit(&nusdt_token_contract, &bob, usdt_bobs_amount / extra_decimals_mult));

    let account = burrowland_contract.get_account(&bob).await?.unwrap();
    assert!(find_asset(&account.supplied, wrap_token_contract.0.id()).apr > BigDecimal::zero());
    assert!(find_asset(&account.supplied, nusdt_token_contract.0.id()).apr > BigDecimal::zero());

    // Assuming 2% discount for NEAR at 12$. Paying 49 USDT for 50 USDC.
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1200000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    let usdt_amount_in = d(49, 18);
    let usdc_amount_out = d(50, 18);
    check!(burrowland_contract.liquidate_with_pyth(&bob, alice.id(), 
    vec![asset_amount(nusdt_token_contract.0.id(), usdt_amount_in)], vec![asset_amount(nusdc_token_contract.0.id(), usdc_amount_out)], None, None), "The health factor of liquidation account can't decrease");

    // Assuming ~2% discount for 5 NEAR at 12$. 50 USDT -> ~51 USDC, 4.9 NEAR -> 60 USDC.
    let wnear_amount_in = d(49, 23);
    let usdt_amount_in = d(50, 18);
    let usdc_amount_out = d(111, 18);
    let outcome = burrowland_contract.liquidate_with_pyth(&bob, alice.id(),
    vec![asset_amount(wrap_token_contract.0.id(), wnear_amount_in), asset_amount(nusdt_token_contract.0.id(), usdt_amount_in)], vec![asset_amount(nusdc_token_contract.0.id(), usdc_amount_out)], None, None).await?;

    let logs = outcome.logs();
    let event = &logs[4];
    assert!(event.starts_with(EVENT_JSON));

    let value: serde_json::Value =
        serde_json::from_str(&event[EVENT_JSON.len()..]).expect("Failed to parse the event");
    assert_eq!(value["standard"].as_str().unwrap(), "burrow");
    assert_eq!(value["event"].as_str().unwrap(), "liquidate");
    assert_eq!(
        value["data"][0]["account_id"].as_str().unwrap(),
        bob.id().as_str()
    );
    assert_eq!(
        value["data"][0]["liquidation_account_id"].as_str().unwrap(),
        alice.id().as_str()
    );
    assert_eq!(
        value["data"][0]["collateral_sum"].as_str().unwrap(),
        "111.0"
    );
    assert_eq!(value["data"][0]["repaid_sum"].as_str().unwrap(), "108.8");

    let account = burrowland_contract.get_account(&alice).await?.unwrap();
    assert!(account.supplied.is_empty());
    assert!(find_asset(&account.borrowed, wrap_token_contract.0.id()).apr > BigDecimal::zero());

    let account = burrowland_contract.get_account(&bob).await?.unwrap();
    assert!(find_asset(&account.supplied, &wrap_token_contract.0.id()).apr > BigDecimal::zero());
    // Now APR should be 0, since Bob has liquidated the entire USDT amount
    assert_eq!(
        find_asset(&account.supplied, &nusdt_token_contract.0.id()).apr,
        BigDecimal::zero()
    );
    assert_eq!(
        find_asset(&account.supplied, &nusdc_token_contract.0.id()).apr,
        BigDecimal::zero()
    );
    Ok(())
}


#[tokio::test]
async fn test_force_close_with_pyth() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let pyth_contract = deploy_mock_pyth(&root).await?;

    let nusdc_token_contract = deploy_mock_ft(&root, "nusdc", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    let wrap_reserve_amount = d(10000, 24);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount));

    let burrowland_contract = deploy_burrowland_with_pyth(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &nusdc_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(nusdc_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdc_token_contract.ft_mint(&root, &alice, supply_amount));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    check!(burrowland_contract.supply_to_collateral(&nusdc_token_contract, &alice, (supply_amount / extra_decimals_mult).into()));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, None));
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1000000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    // usdc
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, nusdc_token_contract.0.id(), 6, 4, "1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", None, None));
    check!(pyth_contract.set_price("1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", PythPrice{
        price: I64(100000000),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    let borrow_amount = d(50, 24);
    check!(burrowland_contract.borrow_and_withdraw_with_pyth(&alice, wrap_token_contract.0.id(), borrow_amount));

   // Attempt to force close the account with NEAR at 12$, the account debt is still not bad.
   let bob = create_account(&root, "bob", None).await;
   check!(burrowland_contract.storage_deposit(&bob));
   let current_timestamp = worker.view_block().await?.timestamp();
   check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
       price: I64(1200000000),
       conf: U64(278100),
       expo: -8,
       publish_time: nano_to_sec(current_timestamp) as i64,
   }));
   check!(burrowland_contract.force_close_with_pyth(&bob, alice.id(), None, None), "is not greater than total collateral");

    // Force closing account with NEAR at 25$.
    // near
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(2500000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    let outcome = burrowland_contract.force_close_with_pyth(&bob, alice.id(), None, None).await?;

    let logs = outcome.logs();
    let event = &logs[4];
    assert!(event.starts_with(EVENT_JSON));

    let value: serde_json::Value =
        serde_json::from_str(&event[EVENT_JSON.len()..]).expect("Failed to parse the event");
    assert_eq!(value["standard"].as_str().unwrap(), "burrow");
    assert_eq!(value["event"].as_str().unwrap(), "force_close");
    assert_eq!(
        value["data"][0]["liquidation_account_id"].as_str().unwrap(),
        alice.id().as_str()
    );
    assert_eq!(
        value["data"][0]["collateral_sum"].as_str().unwrap(),
        "1000.0"
    );
    assert_eq!(value["data"][0]["repaid_sum"].as_str().unwrap()[..6].as_bytes(), "1250.0".as_bytes());

    let account = burrowland_contract.get_account(&alice).await?.unwrap();

    assert!(account.supplied.is_empty());
    assert!(account.collateral.is_empty());
    assert!(account.borrowed.is_empty());
    Ok(())
}

#[tokio::test]
async fn test_force_close_with_pyth_and_default_price() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let pyth_contract = deploy_mock_pyth(&root).await?;

    let nusdc_token_contract = deploy_mock_ft(&root, "nusdc", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    let wrap_reserve_amount = d(10000, 24);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount));

    let burrowland_contract = deploy_burrowland_with_pyth(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &nusdc_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(nusdc_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));
    check!(burrowland_contract.add_token_pyth_info(&root, nusdc_token_contract.0.id(), 6, 4, "1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", None, None));
    // usdc
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(pyth_contract.set_price("1fc18861232290221461220bd4e2acd1dcdfbc89c84092c93c18bdc7756c1588", PythPrice{
        price: I64(100000000),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    
    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdc_token_contract.ft_mint(&root, &alice, supply_amount));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    check!(burrowland_contract.supply_to_collateral(&nusdc_token_contract, &alice, (supply_amount / extra_decimals_mult).into()));

    let borrow_amount = d(50, 24);
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, Some(Price {
        multiplier: 100000,
        decimals: 28,
    })));
    check!(burrowland_contract.borrow_and_withdraw_with_pyth(&alice, wrap_token_contract.0.id(), borrow_amount));

    // Attempt to force close the account with NEAR at 12$, the account debt is still not bad.
    check!(burrowland_contract.update_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, Some(Price {
        multiplier: 120000,
        decimals: 28,
    })));
    let bob = create_account(&root, "bob", None).await;
    check!(burrowland_contract.storage_deposit(&bob));
    check!(burrowland_contract.force_close_with_pyth(&bob, alice.id(), None, None), "is not greater than total collateral");

        // Force closing account with NEAR at 25$.
    // near
    check!(burrowland_contract.update_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, Some(Price {
        multiplier: 250000,
        decimals: 28,
    })));
    
    let outcome = burrowland_contract.force_close_with_pyth(&bob, alice.id(), None, None).await?;

    let logs = outcome.logs();
    println!("{:#?}", logs);
    let event = &logs[4];
    assert!(event.starts_with(EVENT_JSON));

    let value: serde_json::Value =
        serde_json::from_str(&event[EVENT_JSON.len()..]).expect("Failed to parse the event");
    assert_eq!(value["standard"].as_str().unwrap(), "burrow");
    assert_eq!(value["event"].as_str().unwrap(), "force_close");
    assert_eq!(
        value["data"][0]["liquidation_account_id"].as_str().unwrap(),
        alice.id().as_str()
    );
    assert_eq!(
        value["data"][0]["collateral_sum"].as_str().unwrap(),
        "1000.0"
    );
    assert_eq!(value["data"][0]["repaid_sum"].as_str().unwrap()[..6].as_bytes(), "1250.0".as_bytes());

    let account = burrowland_contract.get_account(&alice).await?.unwrap();

    assert!(account.supplied.is_empty());
    assert!(account.collateral.is_empty());
    assert!(account.borrowed.is_empty());
    Ok(())
}

#[tokio::test]
async fn test_batch_actions() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let pyth_contract = deploy_mock_pyth(&root).await?;

    let nusdc_token_contract = deploy_mock_ft(&root, "nusdc", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    let wrap_reserve_amount = d(10000, 24);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount));

    let burrowland_contract = deploy_burrowland_with_pyth(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &nusdc_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(nusdc_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdc_token_contract.ft_mint(&root, &alice, supply_amount));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    // near
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, None));
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(278100000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    // usdc
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, nusdc_token_contract.0.id(), 6, 4, "41f3625971ca2ed2263e78573fe5ce23e13d2558ed3f2e47ab0f84fb9e7ae722", None, None));
    check!(pyth_contract.set_price("41f3625971ca2ed2263e78573fe5ce23e13d2558ed3f2e47ab0f84fb9e7ae722", PythPrice{
        price: I64(99980647),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    let borrow_amount = d(50, 24);
    check!(print burrowland_contract.deposit_increase_collateral_borrow_withdraw_with_pyth(&nusdc_token_contract, &alice, (supply_amount / extra_decimals_mult).into(), wrap_token_contract.0.id(), borrow_amount));

    println!("{:?}", burrowland_contract.get_account_all_positions(&alice).await?.unwrap());

    check!(print burrowland_contract.deposit_repay_decrease_collateral_withdraw_with_pyth(&wrap_token_contract, &alice, (borrow_amount / 2).into(), wrap_token_contract.0.id(), borrow_amount / 2, nusdc_token_contract.0.id(), d(500, 18)));

    println!("{:?}", burrowland_contract.get_account_all_positions(&alice).await?.unwrap());
    
    Ok(())
}

#[tokio::test]
async fn test_switch_price_oracle() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let pyth_contract = deploy_mock_pyth(&root).await?;

    let nusdc_token_contract = deploy_mock_ft(&root, "nusdc", 18).await?;
    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    let wrap_reserve_amount = d(10000, 24);
    check!(wrap_token_contract.ft_mint(&root, &root, wrap_reserve_amount));

    let oracle_contract = deploy_oralce(&root).await?;
    let burrowland_contract = deploy_burrowland_with_pyth(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &nusdc_token_contract));
    check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));
    check!(nusdc_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, wrap_reserve_amount));

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    let supply_amount = d(1000, 18);
    let extra_decimals_mult = d(1, 12);
    check!(nusdc_token_contract.ft_mint(&root, &alice, supply_amount * 10));
    check!(wrap_token_contract.ft_storage_deposit(alice.id()));

    check!(burrowland_contract.supply_to_collateral(&nusdc_token_contract, &alice, (supply_amount / extra_decimals_mult).into()));
    
    check!(burrowland_contract.enable_oracle(&root, false, false), "Only one oracle can be started at a time");
    check!(burrowland_contract.enable_oracle(&root, true, true), "Only one oracle can be started at a time");
    check!(burrowland_contract.enable_oracle(&root, true, false));
    assert!(burrowland_contract.get_config().await?.enable_price_oracle);
    assert!(!burrowland_contract.get_config().await?.enable_pyth_oracle);

    // near
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, None));
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(278100000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    check!(view pyth_contract.get_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4"));

    // usdc
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(burrowland_contract.add_token_pyth_info(&root, nusdc_token_contract.0.id(), 6, 4, "41f3625971ca2ed2263e78573fe5ce23e13d2558ed3f2e47ab0f84fb9e7ae722", None, None));
    check!(pyth_contract.set_price("41f3625971ca2ed2263e78573fe5ce23e13d2558ed3f2e47ab0f84fb9e7ae722", PythPrice{
        price: I64(99980647),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    check!(view pyth_contract.get_price("41f3625971ca2ed2263e78573fe5ce23e13d2558ed3f2e47ab0f84fb9e7ae722"));

    let borrow_amount = d(50, 24);
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(view burrowland_contract.get_account_all_positions(&alice));
    check!(burrowland_contract.deposit_increase_collateral_borrow_withdraw_with_pyth(&nusdc_token_contract, &alice, (supply_amount / extra_decimals_mult).into(), wrap_token_contract.0.id(), borrow_amount), "Pyth oracle disabled");
    check!(burrowland_contract.borrow_and_withdraw(&alice, &oracle_contract, burrowland_contract.0.id(), price_data(current_timestamp, Some(100000)), wrap_token_contract.0.id(), borrow_amount));
    check!(burrowland_contract.enable_oracle(&root, false, true));
    check!(burrowland_contract.borrow_and_withdraw(&alice, &oracle_contract, burrowland_contract.0.id(), price_data(current_timestamp, Some(100000)), wrap_token_contract.0.id(), borrow_amount), "Price oracle disabled");
    check!(burrowland_contract.deposit_increase_collateral_borrow_withdraw_with_pyth(&nusdc_token_contract, &alice, (supply_amount / extra_decimals_mult).into(), wrap_token_contract.0.id(), borrow_amount));
    Ok(())
}

#[tokio::test]
async fn test_repay_old_eth() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let pyth_contract = deploy_mock_pyth(&root).await?;

    let old_eth_contract = deploy_mock_ft(&root, "aurora", 18).await?;
    let new_eth_contract = deploy_mock_ft(&root, "eth", 18).await?;
    let old_eth_reserve_amount = d(10000, 24);
    check!(old_eth_contract.ft_mint(&root, &root, old_eth_reserve_amount));

    let burrowland_contract = deploy_burrowland_with_pyth(&root).await?;
    check!(burrowland_contract.add_asset_handler(&root, &old_eth_contract));
    check!(burrowland_contract.add_asset_handler(&root, &new_eth_contract));
    check!(old_eth_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(new_eth_contract.ft_storage_deposit(burrowland_contract.0.id()));
    check!(burrowland_contract.deposit_to_reserve(&old_eth_contract, &root, old_eth_reserve_amount));

    let alice = create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    let supply_amount = d(1000, 18);
    check!(old_eth_contract.ft_mint(&root, &alice, supply_amount));
    check!(new_eth_contract.ft_mint(&root, &alice, d(5, 17)));

    let current_timestamp = worker.view_block().await?.timestamp();
    // old eth
    check!(burrowland_contract.add_token_pyth_info(&root, old_eth_contract.0.id(), 18, 4, "ca80ba6dc32e08d06f1aa886011eed1d77c77be9eb761cc10d72b7d0a2fd57a6", None, None));
    // new eth
    check!(burrowland_contract.add_token_pyth_info(&root, new_eth_contract.0.id(), 18, 4, "ca80ba6dc32e08d06f1aa886011eed1d77c77be9eb761cc10d72b7d0a2fd57a6", None, None));
    check!(pyth_contract.set_price("ca80ba6dc32e08d06f1aa886011eed1d77c77be9eb761cc10d72b7d0a2fd57a6", PythPrice{
        price: I64(278100000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    let borrow_amount = d(1, 18);
    check!(burrowland_contract.deposit_increase_collateral_borrow_withdraw_with_pyth(&old_eth_contract, &alice, supply_amount, old_eth_contract.0.id(), borrow_amount));

    let alice_acc = burrowland_contract.get_account_all_positions(&alice).await?.unwrap();
    let alice_reg_pos = alice_acc.positions.get(REGULAR_POSITION).unwrap();
    assert_eq!(alice_reg_pos.collateral[0].token_id.as_str(), "aurora.test.near");
    assert_eq!(alice_reg_pos.collateral[0].balance / d(1, 18), 1000);
    assert_eq!(alice_reg_pos.borrowed[0].token_id.as_str(), "aurora.test.near");
    assert_eq!(alice_reg_pos.borrowed[0].balance / d(1, 18), 1);

    check!(burrowland_contract.deposit_repay(&new_eth_contract, &alice, borrow_amount / 2, old_eth_contract.0.id(), borrow_amount / 2));


    let alice_acc = burrowland_contract.get_account_all_positions(&alice).await?.unwrap();
    let alice_reg_pos = alice_acc.positions.get(REGULAR_POSITION).unwrap();
    assert_eq!(alice_reg_pos.collateral[0].token_id.as_str(), "aurora.test.near");
    assert_eq!(alice_reg_pos.collateral[0].balance / d(1, 18), 1000);
    assert_eq!(alice_reg_pos.borrowed[0].token_id.as_str(), "aurora.test.near");
    assert_eq!(alice_reg_pos.borrowed[0].balance / d(1, 17), 5);
    
    Ok(())
}

#[tokio::test]
async fn test_liquidate_old_eth() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;

    let pyth_contract = deploy_mock_pyth(&root).await?;

    let wrap_token_contract = deploy_mock_ft(&root, "wrap", 24).await?;
    let old_eth_token_contract = deploy_mock_ft(&root, "aurora", 18).await?;
    let new_eth_token_contract = deploy_mock_ft(&root, "eth", 18).await?;
    
    let burrowland_contract = deploy_burrowland_with_pyth(&root).await?;
    {
        check!(wrap_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
        check!(old_eth_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
        check!(new_eth_token_contract.ft_storage_deposit(burrowland_contract.0.id()));
        check!(burrowland_contract.add_asset_handler(&root, &old_eth_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &new_eth_token_contract));
        check!(burrowland_contract.add_asset_handler(&root, &wrap_token_contract));

        check!(wrap_token_contract.ft_mint(&root, &root, parse_near!("10000 N")));
        check!(old_eth_token_contract.ft_mint(&root, &root, parse_near!("10000 N")));
        check!(burrowland_contract.deposit_to_reserve(&wrap_token_contract, &root, parse_near!("10000 N")));
        check!(burrowland_contract.deposit_to_reserve(&old_eth_token_contract, &root, parse_near!("10000 N")));
        check!(burrowland_contract.storage_deposit(&root));
        // old eth
        check!(burrowland_contract.add_token_pyth_info(&root, old_eth_token_contract.0.id(), 18, 4, "ca80ba6dc32e08d06f1aa886011eed1d77c77be9eb761cc10d72b7d0a2fd57a6", None, None));
        // new eth
        check!(burrowland_contract.add_token_pyth_info(&root, new_eth_token_contract.0.id(), 18, 4, "ca80ba6dc32e08d06f1aa886011eed1d77c77be9eb761cc10d72b7d0a2fd57a6", None, None));
        // near
        check!(burrowland_contract.add_token_pyth_info(&root, wrap_token_contract.0.id(), 24, 4, "27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", None, None));
    }

    let alice = tool_create_account(&root, "alice", None).await;
    check!(burrowland_contract.storage_deposit(&alice));
    let bob = tool_create_account(&root, "bob", None).await;
    check!(burrowland_contract.storage_deposit(&bob));
    
    assert!(wrap_token_contract.ft_mint(&root, &alice, d(10000, 24)).await?.is_success());
    check!(old_eth_token_contract.ft_storage_deposit(alice.id()));

    
    let current_timestamp = worker.view_block().await?.timestamp();
    check!(pyth_contract.set_price("ca80ba6dc32e08d06f1aa886011eed1d77c77be9eb761cc10d72b7d0a2fd57a6", PythPrice{
        price: I64(100000000),
        conf: U64(103853),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(1000000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    check!(burrowland_contract.deposit_increase_collateral_borrow_withdraw_with_pyth(&wrap_token_contract, &alice, d(10000, 24), old_eth_token_contract.0.id(), d(10000, 18)));
    
    check!(new_eth_token_contract.ft_mint(&root, &bob, d(1, 18)));
    check!(burrowland_contract.deposit(&new_eth_token_contract, &bob, d(1, 18)));

    let current_timestamp = worker.view_block().await?.timestamp();
    check!(pyth_contract.set_price("27e867f0f4f61076456d1a73b14c7edc1cf5cef4f4d6193a33424288f11bd0f4", PythPrice{
        price: I64(100000000),
        conf: U64(278100),
        expo: -8,
        publish_time: nano_to_sec(current_timestamp) as i64,
    }));

    let alice_acc = burrowland_contract.get_account_all_positions(&alice).await?.unwrap();
    let alice_reg_pos = alice_acc.positions.get(REGULAR_POSITION).unwrap();
    assert_eq!(alice_reg_pos.collateral[0].token_id.as_str(), "wrap.test.near");
    assert_eq!(alice_reg_pos.collateral[0].balance / d(1, 24), 10000);
    assert_eq!(alice_reg_pos.borrowed[0].token_id.as_str(), "aurora.test.near");
    assert_eq!(alice_reg_pos.borrowed[0].balance / d(1, 18), 10000);

    let bob_acc = burrowland_contract.get_account_all_positions(&bob).await?.unwrap();
    assert_eq!(bob_acc.supplied[0].token_id.as_str(), "eth.test.near");
    assert_eq!(bob_acc.supplied[0].balance, d(1, 18));
    assert!(bob_acc.positions.is_empty());

    check!(burrowland_contract.liquidate_with_pyth(&bob, alice.id(), 
    vec![asset_amount(old_eth_token_contract.0.id(), d(1, 18))], vec![asset_amount(&wrap_token_contract.0.id(), d(1, 23))], None, None));

    let alice_acc = burrowland_contract.get_account_all_positions(&alice).await?.unwrap();
    let alice_reg_pos = alice_acc.positions.get(REGULAR_POSITION).unwrap();
    assert_eq!(alice_reg_pos.collateral[0].token_id.as_str(), "wrap.test.near");
    assert_eq!(alice_reg_pos.collateral[0].balance / d(1, 23), 99999);
    assert_eq!(alice_reg_pos.borrowed[0].token_id.as_str(), "aurora.test.near");
    assert_eq!(alice_reg_pos.borrowed[0].balance / d(1, 18), 9999);

    let bob_acc = burrowland_contract.get_account_all_positions(&bob).await?.unwrap();
    assert_eq!(bob_acc.supplied[0].token_id.as_str(), "wrap.test.near");
    assert_eq!(bob_acc.supplied[0].balance, d(1, 23));
    assert!(bob_acc.positions.is_empty());
    Ok(())
}