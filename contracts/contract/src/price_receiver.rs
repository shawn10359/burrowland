use crate::*;
use near_sdk::serde_json;

#[derive(Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Serialize))]
#[serde(crate = "near_sdk::serde")]
pub enum PriceReceiverMsg {
    Execute { actions: Vec<Action> },
    MarginExecute { actions: Vec<MarginAction> },
}

impl Contract {
    pub fn validate_price_data(&self, data: &PriceData) {
        let config = self.internal_config();
        assert!(
            data.recency_duration_sec <= config.maximum_recency_duration_sec,
            "Recency duration in the oracle call is larger than allowed maximum"
        );
        let timestamp = env::block_timestamp();
        assert!(
            data.timestamp <= timestamp,
            "Price data timestamp is in the future"
        );
        assert!(
            timestamp - data.timestamp <= to_nano(config.maximum_staleness_duration_sec),
            "Price data timestamp is too stale"
        );
    }
}

#[near_bindgen]
impl OraclePriceReceiver for Contract {
    /// The method will execute a given list of actions in the msg using the prices from the `data`
    /// provided by the oracle on behalf of the sender_id.
    /// - Requires to be called by the oracle account ID.
    fn oracle_on_call(&mut self, sender_id: AccountId, data: PriceData, msg: String) {
        assert_eq!(env::predecessor_account_id(), self.get_oracle_account_id());

        assert!(self.get_config().enable_price_oracle, "Price oracle disabled");

        match serde_json::from_str(&msg).expect("Can't parse PriceReceiverMsg") {
            PriceReceiverMsg::Execute { actions } => {
                self.assert_operation_allowed_in_current_mode(&sender_id);
                let mut account = self.internal_unwrap_account(&sender_id);
                self.validate_price_data(&data);
                self.internal_execute(&sender_id, &mut account, actions, data.into());
                self.internal_set_account(&sender_id, account);
            },
            PriceReceiverMsg::MarginExecute { .. } => {
                env::panic_str(MARGIN_CLOSED_ENTRYPOINT_MSG);
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unit_env::*;

    #[test]
    #[should_panic(expected = "Margin entrypoint is closed")]
    fn test_oracle_margin_execute_is_closed() {
        let mut test_env = init_unit_env();
        let msg = serde_json::to_string(&PriceReceiverMsg::MarginExecute {
            actions: vec![MarginAction::Withdraw {
                token_id: neth_token_id(),
                amount: None,
            }],
        })
        .unwrap();

        test_env.contract_oracle_call(alice(), unit_price_data(0, Some(10000), Some(10000)), msg);
    }
}
