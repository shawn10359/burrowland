use crate::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, Deserialize))]
#[serde(crate = "near_sdk::serde")]
pub struct PendingMarginAction {
    pub dex_id: AccountId,
    pub token_out: TokenId,
    pub op: String,
    pub amount_in: U128,
    pub min_amount_out: U128,
    pub liquidator_id: Option<AccountId>,
}

fn pending_action_storage_key(account_id: &AccountId, pos_id: &PosId) -> Vec<u8> {
    format!("{}:{}:{}", MARGIN_PENDING_ACTION_PREFIX, account_id, pos_id).into_bytes()
}

pub fn read_pending_margin_action(account_id: &AccountId, pos_id: &PosId) -> Option<PendingMarginAction> {
    env::storage_read(&pending_action_storage_key(account_id, pos_id)).map(|v| {
        PendingMarginAction::try_from_slice(&v).expect("deserialize pending margin action failed")
    })
}

pub fn write_pending_margin_action(account_id: &AccountId, pos_id: &PosId, data: &PendingMarginAction) {
    env::storage_write(
        &pending_action_storage_key(account_id, pos_id),
        &data.try_to_vec().unwrap(),
    );
}

pub fn remove_pending_margin_action(account_id: &AccountId, pos_id: &PosId) {
    env::storage_remove(&pending_action_storage_key(account_id, pos_id));
}
