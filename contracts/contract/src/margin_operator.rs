use crate::*;

pub fn read_margin_operator_from_storage() -> Option<AccountId> {
    env::storage_read(MARGIN_OPERATOR_KEY.as_bytes())
        .map(|content| AccountId::try_from_slice(&content).expect("deserialize margin operator failed"))
}

pub fn write_margin_operator_to_storage(operator_id: &AccountId) {
    env::storage_write(
        MARGIN_OPERATOR_KEY.as_bytes(),
        &operator_id.try_to_vec().unwrap(),
    );
}

pub fn is_margin_operator(operator_id: &AccountId) -> bool {
    read_margin_operator_from_storage().map_or(false, |stored_operator_id| stored_operator_id == *operator_id)
}

pub fn assert_margin_operator_close_action(action: &MarginAction) {
    require!(
        matches!(action, MarginAction::CloseMTPosition { .. }),
        "Only CloseMTPosition is allowed for margin operator"
    );
}

#[near_bindgen]
impl Contract {
    #[payable]
    pub fn set_margin_operator(&mut self, operator_id: AccountId) {
        assert_one_yocto();
        self.assert_owner();

        write_margin_operator_to_storage(&operator_id);
    }

    pub fn get_margin_operator(&self) -> Option<AccountId> {
        read_margin_operator_from_storage()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unit_env::*;

    fn account(account_id: &str) -> AccountId {
        AccountId::new_unchecked(account_id.to_string())
    }

    #[test]
    fn test_set_margin_operator() {
        let mut test_env = init_unit_env();
        let operator_id = account("operator.near");

        test_env
            .contract
            .set_margin_operator(operator_id.clone());

        assert_eq!(test_env.contract.get_margin_operator(), Some(operator_id.clone()));
        assert!(is_margin_operator(&operator_id));
    }

    #[test]
    fn test_set_margin_operator_overwrites_existing_operator() {
        let mut test_env = init_unit_env();
        let operator_id = account("operator.near");
        let next_operator_id = account("next-operator.near");

        test_env
            .contract
            .set_margin_operator(operator_id.clone());
        test_env
            .contract
            .set_margin_operator(next_operator_id.clone());

        assert_eq!(test_env.contract.get_margin_operator(), Some(next_operator_id.clone()));
        assert!(!is_margin_operator(&operator_id));
        assert!(is_margin_operator(&next_operator_id));
    }

    #[test]
    fn test_missing_margin_operator_storage_returns_false() {
        assert!(!is_margin_operator(&account("operator.near")));
    }
}
