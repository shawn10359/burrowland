use crate::*;
use near_contract_standards::fungible_token::core::ext_ft_core;
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::json_types::U128;
use near_sdk::{is_promise_success, serde_json, PromiseOrValue};

const GAS_FOR_FT_TRANSFER: Gas = Gas(Gas::ONE_TERA.0 * 10);
const GAS_FOR_AFTER_FT_TRANSFER: Gas = Gas(Gas::ONE_TERA.0 * 10);

#[derive(Deserialize, Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
#[serde(crate = "near_sdk::serde")]
pub enum TokenReceiverMsg {
    Execute { actions: Vec<Action> },
    ExecuteWithPyth { actions: Vec<Action> },
    DepositToReserve,
    DepositToMargin,
    MarginExecute { actions: Vec<MarginAction> },
    MarginExecuteWithPyth { actions: Vec<MarginAction> },
    SwapReference { swap_ref: SwapReference },
    OnlyIncreaseCollateral,
    OnlyRepay,
    ClientEchoDeposit { client_echo: String },
}

#[near_bindgen]
impl FungibleTokenReceiver for Contract {
    /// Receives the transfer from the fungible token and executes a list of actions given in the
    /// message on behalf of the sender. The actions that can be executed should be limited to a set
    /// that doesn't require pricing.
    /// - Requires to be called by the fungible token account.
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        // Set reliable liquidator context if sender is in whitelist
        self.is_reliable_liquidator_context = in_reliable_liquidator_whitelist(&sender_id.to_string());

        let token_id = env::predecessor_account_id();
        let mut asset = self.internal_unwrap_asset(&token_id);

        let ft_amount = amount.0;
        let amount = amount.0 * 10u128.pow(asset.config.extra_decimals as u32);

        let (actions, with_pyth) = if msg.is_empty() {
            self.assert_operation_allowed_in_current_mode(&sender_id);
            self.assert_ft_transfer_deposit_enabled(&asset);
            (vec![], false)
        } else {
            let token_receiver_msg: TokenReceiverMsg =
                serde_json::from_str(&msg).expect("Can't parse TokenReceiverMsg");
            match token_receiver_msg {
                TokenReceiverMsg::Execute { actions } => {
                    self.assert_operation_allowed_in_current_mode(&sender_id);
                    self.assert_ft_transfer_deposit_enabled(&asset);
                    (actions, false)
                },
                TokenReceiverMsg::ExecuteWithPyth { actions } => {
                    self.assert_operation_allowed_in_current_mode(&sender_id);
                    self.assert_ft_transfer_deposit_enabled(&asset);
                    (actions, true)
                },
                TokenReceiverMsg::DepositToReserve => {
                    self.assert_operation_allowed_in_current_mode(&sender_id);
                    self.assert_ft_transfer_deposit_enabled(&asset);
                    let mut protocol_debts = read_protocol_debts_from_storage();
                    let amount_to_reserved = if let Some(debt) = protocol_debts.remove(&token_id) {
                        let repay_amount = std::cmp::min(amount, debt);
                        let remain_debt = debt - repay_amount;
                        if remain_debt > 0 {
                            protocol_debts.insert(token_id.clone(), remain_debt);
                        }
                        write_protocol_debts_to_storage(protocol_debts);
                        events::emit::repay_protocol_debts(&token_id, repay_amount);
                        amount - repay_amount
                    } else {
                        amount
                    };
                    asset.reserved += amount_to_reserved;
                    self.internal_set_asset(&token_id, asset);
                    events::emit::deposit_to_reserve(&sender_id, amount, &token_id);
                    return PromiseOrValue::Value(U128(0));
                }
                TokenReceiverMsg::DepositToMargin => {
                    env::panic_str(MARGIN_CLOSED_ENTRYPOINT_MSG);
                }
                TokenReceiverMsg::MarginExecute { .. } => {
                    env::panic_str(MARGIN_CLOSED_ENTRYPOINT_MSG);
                }
                TokenReceiverMsg::MarginExecuteWithPyth { .. } => {
                    env::panic_str(MARGIN_CLOSED_ENTRYPOINT_MSG);
                }
                TokenReceiverMsg::SwapReference { .. } => {
                    env::panic_str(MARGIN_CLOSED_ENTRYPOINT_MSG);
                }
                TokenReceiverMsg::OnlyIncreaseCollateral => {
                    self.assert_operation_allowed_in_current_mode(&sender_id);
                    self.assert_ft_transfer_deposit_enabled(&asset);
                    self.internal_only_increase_collateral(&sender_id, &token_id, amount);
                    return PromiseOrValue::Value(U128(0));
                }
                TokenReceiverMsg::OnlyRepay => {
                    self.assert_operation_allowed_in_current_mode(&sender_id);
                    self.assert_ft_transfer_deposit_enabled(&asset);
                    self.internal_only_repay(&sender_id, &token_id, amount);
                    return PromiseOrValue::Value(U128(0));
                }
                TokenReceiverMsg::ClientEchoDeposit { client_echo } => {
                    self.assert_operation_allowed_in_current_mode(&sender_id);
                    self.assert_ft_transfer_deposit_enabled(&asset);
                    assert!(in_client_echo_sender_whitelist(sender_id.as_str()), "Unauthorized client echo sender: {}", sender_id);
                    let mut account = self.internal_unwrap_account(&sender_id);
                    assert!(!account.is_locked, "Account is locked!");
                    let supplied_shares = self.internal_deposit(&mut account, &token_id, amount);
                    events::emit::deposit(&sender_id, amount, &token_id);
                    self.internal_account_apply_affected_farms(&mut account);
                    self.internal_set_account(&sender_id, account);
                    Promise::new(sender_id.clone())
                        .function_call_weight(
                            "on_burrowland_supply_client_echo".to_string(),
                            serde_json::json!({
                                "token_id": token_id,
                                "supplied_shares": supplied_shares,
                                "supplied_ft_amount": U128(ft_amount),
                                "msg": client_echo,
                            })
                            .to_string()
                            .into_bytes(),
                            0,
                            Gas(Gas::ONE_TERA.0 * 50),
                            Default::default()
                        );
                    return PromiseOrValue::Value(U128(0));
                }
            }
        };

        let mut account = self.internal_unwrap_account(&sender_id);
        assert!(!account.is_locked, "Account is locked!");
        self.internal_deposit(&mut account, &token_id, amount);
        events::emit::deposit(&sender_id, amount, &token_id);
        if with_pyth {
            self.internal_execute_with_pyth(&sender_id, &mut account, actions);
        } else {
            self.internal_execute(&sender_id, &mut account, actions, Prices::new());
        }
        self.internal_set_account(&sender_id, account);

        PromiseOrValue::Value(U128(0))
    }
}

impl Contract {
    fn assert_ft_transfer_deposit_enabled(&self, asset: &Asset) {
        assert!(
            asset.config.can_deposit,
            "Deposits for this asset are not enabled"
        );
    }

    pub fn internal_beneficiary_withdraw(
        &mut self, 
        account_id: &AccountId, 
        token_id: &TokenId, 
        amount: Balance, 
        ft_amount: Balance,
    ) -> Promise {
        ext_ft_core::ext(token_id.clone())
            .with_attached_deposit(ONE_YOCTO)
            .with_static_gas(GAS_FOR_FT_TRANSFER)
            .ft_transfer(account_id.clone(), ft_amount.into(), None)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
                    .with_unused_gas_weight(0)
                    .after_beneficiary_withdraw(account_id.clone(), token_id.clone(), amount.into())
            )
    }

    pub fn internal_ft_transfer(
        &mut self,
        account_id: &AccountId,
        token_id: &TokenId,
        amount: Balance,
        ft_amount: Balance,
        is_margin_asset: bool,
        recipient_id: &AccountId,
    ) -> Promise {
        ext_ft_core::ext(token_id.clone())
            .with_attached_deposit(ONE_YOCTO)
            .with_static_gas(GAS_FOR_FT_TRANSFER)
            .ft_transfer(recipient_id.clone(), ft_amount.into(), None)
            .then(
                if is_margin_asset {
                    Self::ext(env::current_account_id())
                        .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
                        .with_unused_gas_weight(0)
                        .after_margin_asset_ft_transfer(account_id.clone(), token_id.clone(), amount.into())
                } else {
                    Self::ext(env::current_account_id())
                        .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
                        .with_unused_gas_weight(0)
                        .after_ft_transfer(account_id.clone(), token_id.clone(), amount.into())
                }
            )
    }

    pub fn internal_ft_transfer_call(
        &mut self,
        account_id: &AccountId,
        token_id: &TokenId,
        amount: Balance,
        ft_amount: Balance,
        client_echo: String,
    ) {
        ext_ft_core::ext(token_id.clone())
            .with_attached_deposit(ONE_YOCTO)
            .with_static_gas(Gas::ONE_TERA * 30)
            .ft_transfer_call(account_id.clone(), ft_amount.into(), None, client_echo)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
                    .with_unused_gas_weight(0)
                    .after_ft_transfer_call(account_id.clone(), token_id.clone(), ft_amount.into(), amount.into())
            );
    }
}

#[allow(unused)]
#[ext_contract(ext_self)]
trait ExtSelf {
    fn after_beneficiary_withdraw(&mut self, account_id: AccountId, token_id: TokenId, amount: U128)
        -> bool;
    fn after_ft_transfer(&mut self, account_id: AccountId, token_id: TokenId, amount: U128)
        -> bool;
    fn after_ft_transfer_call(
        &mut self,
        account_id: AccountId,
        token_id: TokenId,
        ft_amount: U128,
        amount: U128,
    ) -> Option<(U128, U128)>;
    fn after_margin_asset_ft_transfer(
        &mut self,
        account_id: AccountId,
        token_id: TokenId,
        amount: U128,
    ) -> bool;
}

#[near_bindgen]
impl ExtSelf for Contract {
    #[private]
    fn after_beneficiary_withdraw(
        &mut self,
        account_id: AccountId,
        token_id: TokenId,
        amount: U128,
    ) -> bool {
        let promise_success = is_promise_success();
        if !promise_success {
            let mut asset = self.internal_unwrap_asset(&token_id);
            let old_amount = asset.beneficiary_fees.get(&account_id).unwrap_or(&U128(0)).0;
            asset.beneficiary_fees.insert(account_id.clone(), U128(old_amount + amount.0));
            self.internal_set_asset(&token_id, asset);
            events::emit::withdraw_beneficiary_fee_failed(&account_id, amount.0, &token_id);
        } else {
            events::emit::withdraw_beneficiary_fee_succeeded(&account_id, amount.0, &token_id);
        }
        promise_success
    }

    #[private]
    fn after_ft_transfer(
        &mut self,
        account_id: AccountId,
        token_id: TokenId,
        amount: U128,
    ) -> bool {
        let promise_success = is_promise_success();
        if !promise_success {
            let mut account = self.internal_unwrap_account(&account_id);
            self.internal_deposit_without_asset_basic_check(&mut account, &token_id, amount.0);
            events::emit::withdraw_failed(&account_id, amount.0, &token_id);
            self.internal_force_set_account(&account_id, account);
        } else {
            events::emit::withdraw_succeeded(&account_id, amount.0, &token_id);
        }
        promise_success
    }

    #[private]
    fn after_ft_transfer_call(
        &mut self,
        account_id: AccountId,
        token_id: TokenId,
        ft_amount: U128,
        amount: U128,
    ) -> Option<(U128, U128)> {
        let remain_ft_amount = match promise_result_as_success() {
            Some(result_bytes) => {
                let used_amount = serde_json::from_slice::<U128>(&result_bytes).unwrap();
                ft_amount.0.checked_sub(used_amount.0).unwrap_or(0) 
            }
            None => ft_amount.0,
        };
        if remain_ft_amount == 0 {
            events::emit::withdraw_succeeded(&account_id, amount.0, &token_id);
            None
        } else {
            let remain_amount = u128_ratio(amount.0, remain_ft_amount, ft_amount.0);
            let mut account = self.internal_unwrap_account(&account_id);
            let redeposit_shares = self.internal_deposit_without_asset_basic_check(&mut account, &token_id, remain_amount);
            self.internal_force_set_account(&account_id, account);
            if remain_ft_amount == ft_amount.0 {
                events::emit::withdraw_failed(&account_id, amount.0, &token_id);
            } else {
                events::emit::withdraw_failed(&account_id, remain_amount, &token_id);
                events::emit::withdraw_succeeded(&account_id, amount.0 - remain_amount, &token_id);
            }
            Some((U128(remain_amount), redeposit_shares))
        }
    }

    #[private]
    fn after_margin_asset_ft_transfer(
        &mut self,
        account_id: AccountId,
        token_id: TokenId,
        amount: U128,
    ) -> bool {
        let promise_success = is_promise_success();
        if !promise_success {
            let mut margin_account = self.internal_unwrap_margin_account(&account_id);
            self.internal_margin_deposit_without_asset_basic_check(&mut margin_account, &token_id, amount.0);
            events::emit::margin_asset_withdraw_failed(&account_id, amount.0, &token_id);
            self.internal_force_set_margin_account(&account_id, margin_account);
        } else {
            events::emit::margin_asset_withdraw_succeeded(&account_id, amount.0, &token_id);
        }
        promise_success
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit_env::{alice, bob, d, init_unit_env, ndai_token_id, neth_token_id};

    #[test]
    #[should_panic(expected = "Margin entrypoint is closed")]
    fn test_deposit_to_margin_is_closed() {
        let mut test_env = init_unit_env();
        test_env.contract_ft_transfer_call(
            neth_token_id(),
            alice(),
            U128(d(1, 18)),
            "\"DepositToMargin\"".to_string(),
        );
    }

    #[test]
    #[should_panic(expected = "Margin entrypoint is closed")]
    fn test_margin_execute_on_ft_transfer_is_closed() {
        let mut test_env = init_unit_env();
        let msg = serde_json::to_string(&TokenReceiverMsg::MarginExecute {
            actions: vec![MarginAction::Withdraw {
                token_id: neth_token_id(),
                amount: None,
            }],
        })
        .unwrap();

        test_env.contract_ft_transfer_call(
            neth_token_id(),
            alice(),
            U128(d(1, 18)),
            msg,
        );
    }

    #[test]
    #[should_panic(expected = "Margin entrypoint is closed")]
    fn test_margin_execute_with_pyth_on_ft_transfer_is_closed() {
        let mut test_env = init_unit_env();
        let msg = serde_json::to_string(&TokenReceiverMsg::MarginExecuteWithPyth {
            actions: vec![MarginAction::Withdraw {
                token_id: neth_token_id(),
                amount: None,
            }],
        })
        .unwrap();

        test_env.contract_ft_transfer_call(
            neth_token_id(),
            alice(),
            U128(d(1, 18)),
            msg,
        );
    }

    #[test]
    #[should_panic(expected = "Margin entrypoint is closed")]
    fn test_swap_reference_is_closed() {
        let mut test_env = init_unit_env();
        let msg = serde_json::to_string(&TokenReceiverMsg::SwapReference {
            swap_ref: SwapReference {
                account_id: alice(),
                pos_id: "pos-1".to_string(),
                amount_in: U128(d(1, 18)),
                action_ts: U64(123),
                op: "open".to_string(),
                liquidator_id: None,
            },
        })
        .unwrap();

        test_env.contract_ft_transfer_call(ndai_token_id(), bob(), U128(1), msg);
    }
}
