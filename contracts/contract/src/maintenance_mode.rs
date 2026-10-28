use crate::*;

#[near_bindgen]
impl Contract {
    #[payable]
    pub fn append_maintenance_mode_whitelist(&mut self, account_list: Vec<String>) {
        assert_one_yocto();
        self.assert_owner();
        let mut whitelist: HashSet<String> = if env::storage_has_key(MAINTENANCE_MODE_WHITELIST.as_bytes()) {
            internal_get_maintenance_mode_whitelist()
        } else {
            HashSet::new()
        };
        for account in account_list {
            let is_success = whitelist.insert(account.clone());
            require!(is_success, format!("exist maintenance account: {}", account));
        }
        env::storage_write(
            MAINTENANCE_MODE_WHITELIST.as_bytes(),
            &whitelist.try_to_vec().unwrap(),
        );
    }

    #[payable]
    pub fn remove_maintenance_mode_whitelist(&mut self, account_list: Vec<String>) {
        assert_one_yocto();
        self.assert_owner();
        let mut whitelist = internal_get_maintenance_mode_whitelist();
        for account in account_list {
            let is_success = whitelist.remove(&account);
            require!(is_success, format!("maintenance account {} not exist", account));
        }
        env::storage_write(
            MAINTENANCE_MODE_WHITELIST.as_bytes(),
            &whitelist.try_to_vec().unwrap(),
        );
    }

    pub fn get_maintenance_mode_whitelist(&self) -> Vec<String> {
        if env::storage_has_key(MAINTENANCE_MODE_WHITELIST.as_bytes()) {
            let whitelist = internal_get_maintenance_mode_whitelist();
            whitelist.iter().cloned().collect()
        } else {
            vec![]
        }
    }
}

pub fn internal_get_maintenance_mode_whitelist() -> HashSet<String> {
    let content = env::storage_read(MAINTENANCE_MODE_WHITELIST.as_bytes())
        .expect("Empty storage(MAINTENANCE_MODE_WHITELIST)");
    HashSet::try_from_slice(&content).expect("deserialize maintenance mode whitelist failed.")
}

pub fn in_maintenance_mode_whitelist(account_id: &str) -> bool {
    if !env::storage_has_key(MAINTENANCE_MODE_WHITELIST.as_bytes()) {
        return false;
    }

    internal_get_maintenance_mode_whitelist().iter().any(|item| {
        if let Some(suffix) = item.strip_prefix("*.") {
            account_id.ends_with(&format!(".{}", suffix))
        } else {
            account_id == item
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use unit_env::*;

    #[test]
    fn test_append_maintenance_mode_whitelist() {
        let mut test_env = init_unit_env();
        test_env.contract.append_maintenance_mode_whitelist(vec!["alice.near".to_string(), "bob.near".to_string()]);

        let whitelist = test_env.contract.get_maintenance_mode_whitelist();
        assert_eq!(whitelist.len(), 2);
        assert!(whitelist.contains(&"alice.near".to_string()));
        assert!(whitelist.contains(&"bob.near".to_string()));
    }

    #[test]
    fn test_in_maintenance_mode_whitelist_wildcard_match() {
        let mut test_env = init_unit_env();
        test_env.contract.append_maintenance_mode_whitelist(vec!["*.ops.near".to_string(), "specific.near".to_string()]);

        assert!(in_maintenance_mode_whitelist("alice.ops.near"));
        assert!(in_maintenance_mode_whitelist("specific.near"));
        assert!(!in_maintenance_mode_whitelist("ops.near"));
        assert!(!in_maintenance_mode_whitelist("alice.near"));
    }
}
