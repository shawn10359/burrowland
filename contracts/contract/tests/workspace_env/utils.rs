use crate::*;

use contract::AssetAmount;

pub const EVENT_JSON: &str = "EVENT_JSON:";

pub const EXTRA_CALL_GET_ST_NEAR_PRICE: &str = "get_st_near_price";
pub const EXTRA_CALL_GET_NEARX_PRICE: &str = "get_nearx_price";
pub const EXTRA_CALL_FT_PRICE: &str = "ft_price";

pub fn d(value: Balance, decimals: u8) -> Balance {
    value * 10u128.pow(decimals as _)
}

pub fn asset_amount(token_id: &AccountId, amount: Balance) -> AssetAmount {
    AssetAmount {
        token_id: near_sdk::AccountId::new_unchecked(token_id.to_string()),
        amount: if amount == 0 {None} else {Some(amount.into())},
        max_amount: None,
    }
}

pub async fn create_account(
    master: &Account,
    account_id: &str,
    balance: Option<u128>,
) -> Account {
    let balance = if let Some(balance) = balance {
        NearToken::from_yoctonear(balance)
    } else {
        NearToken::from_near(50)
    };
    master
        .create_subaccount(account_id)
        .initial_balance(balance)
        .transact()
        .await
        .unwrap()
        .unwrap()
}

pub fn tool_err_msg(outcome: &Result<ExecutionFinalResult>) -> String {
    match outcome {
        Ok(res) => {
            let mut msg = "".to_string();
            for r in res.receipt_failures() {
                match r.clone().into_result() {
                    Ok(_) => {}
                    Err(err) => {
                        msg += &format!("{:?}", err);
                        msg += "\n";
                    }
                }
            }
            msg
        }
        Err(err) => err.to_string(),
    }
}

pub fn price_data(
    timestamp: u64,
    wnear_mul: Option<Balance>,
) -> PriceData {
    let mut prices = vec![
        AssetOptionalPrice {
            asset_id: "ndai.test.near".to_string(),
            price: Some(Price {
                multiplier: 10000,
                decimals: 22,
            }),
        },
        AssetOptionalPrice {
            asset_id: "nusdc.test.near".to_string(),
            price: Some(Price {
                multiplier: 10000,
                decimals: 10,
            }),
        },
        AssetOptionalPrice {
            asset_id: "nusdt.test.near".to_string(),
            price: Some(Price {
                multiplier: 10000,
                decimals: 10,
            }),
        },
        AssetOptionalPrice {
            asset_id: "ref.test.near".to_string(),
            price: Some(Price {
                multiplier: 10000,
                decimals: 22,
            }),
        },
    ];
    if let Some(wnear_mul) = wnear_mul {
        prices.push(AssetOptionalPrice {
            asset_id: "wrap.test.near".to_string(),
            price: Some(Price {
                multiplier: wnear_mul,
                decimals: 28,
            }),
        })
    }
    PriceData {
        timestamp,
        recency_duration_sec: 90,
        prices,
    }
}

pub fn av(token_id: &AccountId, balance: Balance) -> AssetView {
    AssetView {
        token_id: near_sdk::AccountId::new_unchecked(token_id.to_string()),
        balance,
        shares: U128(0),
        apr: Default::default(),
    }
}

pub fn find_asset<'a>(assets: &'a [AssetView], token_id: &AccountId) -> &'a AssetView {
    assets
        .iter()
        .find(|e| e.token_id.to_string() == token_id.to_string())
        .expect("Missing asset")
}

pub fn assert_balances(actual: &[AssetView], expected: &[AssetView]) {
    assert_eq!(actual.len(), expected.len());
    for asset in actual {
        assert_eq!(asset.balance, find_asset(expected, &asset.token_id.to_string().parse().unwrap()).balance);
    }
}

pub async fn tool_create_account(
    master: &Account,
    account_id: &str,
    balance: Option<u128>,
) -> Account {
    let balance = if let Some(balance) = balance {
        NearToken::from_yoctonear(balance)
    } else {
        NearToken::from_near(50)
    };
    master
        .create_subaccount(account_id)
        .initial_balance(balance)
        .transact()
        .await
        .unwrap()
        .unwrap()
}

#[macro_export]
macro_rules! check {
    ($exec_func: expr) => {
        let outcome = $exec_func.await.unwrap();
        if !outcome.is_success() || !outcome.receipt_failures().is_empty() {
            let err_msg = tool_err_msg(&Ok(outcome.clone()));
            eprintln!("Transaction failed!");
            eprintln!("Logs: {:#?}", outcome.logs());
            if !err_msg.is_empty() {
                eprintln!("Errors: {}", err_msg);
            }
        }
        assert!(outcome.is_success() && outcome.receipt_failures().is_empty());
    };
    (logs $exec_func: expr) => {
        let outcome = $exec_func.await.unwrap();
        assert!(outcome.is_success() && outcome.receipt_failures().is_empty());
        println!("logs: {:#?}", outcome.logs());
    };
    (print $exec_func: expr) => {
        let outcome = $exec_func.await;
        let err_msg = tool_err_msg(&outcome);
        println!("==>");
        let o = outcome.unwrap();
        println!("logs: {:#?}", o.logs());
        if !err_msg.is_empty() {
            println!("errors: {}", err_msg);
        }
        println!("<==");
    };
    (print $prefix: literal $exec_func: expr) => {
        let outcome = $exec_func.await;
        let err_msg = tool_err_msg(&outcome);
        println!("==>");
        let o = outcome.unwrap();
        println!("{} logs: {:#?}", $prefix, o.logs());
        if !err_msg.is_empty() {
            println!("{} errors: {}", $prefix, err_msg);
        }
        println!("<==");
    };
    (printr $exec_func: expr) => {
        let outcome = $exec_func.await;
        let err_msg = tool_err_msg(&outcome);
        println!("==>");
        if err_msg.is_empty() {
            let o = outcome.unwrap();
            println!("logs: {:#?}", o.logs());
            println!("");
            println!("return: {:#?}", o.json::<near_sdk::serde_json::Value>());
        } else {
            println!("errors: {}", err_msg);
        }
        println!("<==");
    };
    (printr $prefix: literal $exec_func: expr) => {
        let outcome = $exec_func.await;
        let err_msg = tool_err_msg(&outcome);
        println!("==>");
        if err_msg.is_empty() {
            let o = outcome.unwrap();
            println!("{} logs: {:#?}", $prefix, o.logs());
            println!("");
            println!(
                "{} return: {:#?}",
                $prefix,
                o.json::<near_sdk::serde_json::Value>()
            );
        } else {
            println!("{} errors: {}", $prefix, err_msg);
        }
        println!("<==");
    };
    (view $exec_func: expr) => {
        let query_result = $exec_func.await.unwrap();
        println!("{:?}", query_result);
    };
    (view $prefix: literal $exec_func: expr) => {
        let query_result = $exec_func.await.unwrap();
        println!("{} {:#?}", $prefix, query_result);
    };
    ($exec_func: expr, $err_info: expr) => {
        let err_msg = tool_err_msg(&$exec_func.await);
        if !err_msg.contains($err_info) {
            assert!(false, "panic msg: {}", err_msg);
        }
    };
    (logs $exec_func: expr, $err_info: expr) => {
        let outcome = $exec_func.await;
        assert!(tool_err_msg(&outcome).contains($err_info));
        println!("failed logs: {:#?}", outcome.unwrap().logs());
    };
}