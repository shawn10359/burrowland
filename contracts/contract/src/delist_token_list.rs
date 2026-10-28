use crate::*;

#[near_bindgen]
impl Contract {
    #[payable]
    pub fn append_delist_token_list(&mut self, token_list: Vec<String>) {
        assert_one_yocto();
        self.assert_owner();
        let mut delist_token_list: HashSet<String> = if env::storage_has_key(DELIST_TOKEN_LIST.as_bytes()) {
            internal_get_delist_token_list()
        } else {
            HashSet::new()
        };
        for token_id in token_list {
            let is_success = delist_token_list.insert(token_id.clone());
            require!(is_success, format!("exist delist token: {}", token_id));
        }
        env::storage_write(
            DELIST_TOKEN_LIST.as_bytes(),
            &delist_token_list.try_to_vec().unwrap(),
        );
    }

    #[payable]
    pub fn remove_delist_token_list(&mut self, token_list: Vec<String>) {
        assert_one_yocto();
        self.assert_owner();
        let mut delist_token_list = internal_get_delist_token_list();
        for token_id in token_list {
            let is_success = delist_token_list.remove(&token_id);
            require!(is_success, format!("delist token {} not exist", token_id));
        }
        env::storage_write(
            DELIST_TOKEN_LIST.as_bytes(),
            &delist_token_list.try_to_vec().unwrap(),
        );
    }

    pub fn get_delist_token_list(&self) -> Vec<String> {
        if env::storage_has_key(DELIST_TOKEN_LIST.as_bytes()) {
            let delist_token_list = internal_get_delist_token_list();
            delist_token_list.iter().cloned().collect()
        } else {
            vec![]
        }
    }
}

pub fn internal_get_delist_token_list() -> HashSet<String> {
    let content = env::storage_read(DELIST_TOKEN_LIST.as_bytes()).expect("Empty storage(DELIST_TOKEN_LIST)");
    HashSet::try_from_slice(&content).expect("deserialize delist token list failed.")
}

pub fn find_delist_token<'a, I>(token_ids: I) -> Option<&'a TokenId>
where
    I: IntoIterator<Item = &'a TokenId>,
{
    if !env::storage_has_key(DELIST_TOKEN_LIST.as_bytes()) {
        return None;
    }

    let delist_token_list = internal_get_delist_token_list();
    for token_id in token_ids {
        if delist_token_list.contains(token_id.as_str()) {
            return Some(token_id);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use unit_env::*;

    #[test]
    fn test_delist_token_list_exact_match() {
        let mut test_env = init_unit_env();
        testing_env!(test_env.context.predecessor_account_id(owner_id()).attached_deposit(1).build());
        test_env.contract.append_delist_token_list(vec![wnear_token_id().to_string()]);

        let delist_token_list = test_env.contract.get_delist_token_list();
        assert_eq!(delist_token_list.len(), 1);
        assert!(delist_token_list.contains(&wnear_token_id().to_string()));
        let exact_match_tokens = vec![ndai_token_id(), wnear_token_id()];
        assert_eq!(find_delist_token(exact_match_tokens.iter()), Some(&wnear_token_id()));

        let unmatched_tokens = vec![
            AccountId::new_unchecked("token.delist.near".to_string()),
            ndai_token_id(),
        ];
        assert!(find_delist_token(unmatched_tokens.iter()).is_none());
    }

    #[test]
    fn test_remove_delist_token_list() {
        let mut test_env = init_unit_env();
        testing_env!(test_env.context.predecessor_account_id(owner_id()).attached_deposit(1).build());
        test_env.contract.append_delist_token_list(vec![wnear_token_id().to_string()]);
        testing_env!(test_env.context.predecessor_account_id(owner_id()).attached_deposit(1).build());
        test_env.contract.remove_delist_token_list(vec![wnear_token_id().to_string()]);

        assert!(test_env.contract.get_delist_token_list().is_empty());
        let tokens = vec![wnear_token_id()];
        assert!(find_delist_token(tokens.iter()).is_none());
    }
}
