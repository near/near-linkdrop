use near_sdk::store::UnorderedMap;
use near_sdk::{
    AccountId, Allowance, Gas, NearToken, PanicOnDefault, Promise, PromiseResult, PublicKey, env,
    near,
};
use std::num::NonZeroU128;

mod models;
use models::*;

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct LinkDrop {
    pub accounts: UnorderedMap<PublicKey, NearToken>,
}

/// Access key allowance for linkdrop keys.
const ACCESS_KEY_ALLOWANCE: NearToken = NearToken::from_near(1);

/// Gas attached to the callback from account creation.
pub const ON_CREATE_ACCOUNT_CALLBACK_GAS: Gas = Gas::from_tgas(13);

/// Methods callable by the function call access key
const ACCESS_KEY_METHOD_NAMES: &str = "claim,create_account_and_claim";

fn is_promise_success() -> bool {
    assert_eq!(
        env::promise_results_count(),
        1,
        "Contract expected a result on the callback"
    );
    matches!(env::promise_result(0), PromiseResult::Successful(_))
}

#[near]
impl LinkDrop {
    /// Initializes the contract with an empty map for the accounts
    #[init]
    pub fn new() -> Self {
        Self {
            accounts: UnorderedMap::new(b"a"),
        }
    }

    /// Allows given public key to claim sent balance.
    /// Takes ACCESS_KEY_ALLOWANCE as fee from deposit to cover account creation via an access key.
    #[payable]
    pub fn send(&mut self, public_key: PublicKey) -> Promise {
        assert!(
            env::attached_deposit() > ACCESS_KEY_ALLOWANCE,
            "Attached deposit must be greater than ACCESS_KEY_ALLOWANCE"
        );
        let value = self
            .accounts
            .get(&public_key)
            .cloned()
            .unwrap_or(NearToken::from_near(0));
        self.accounts.insert(
            public_key.clone(),
            value
                .saturating_add(env::attached_deposit())
                .saturating_sub(ACCESS_KEY_ALLOWANCE),
        );
        Promise::new(env::current_account_id()).add_access_key_allowance(
            public_key,
            Allowance::Limited(NonZeroU128::new(ACCESS_KEY_ALLOWANCE.as_yoctonear()).unwrap()),
            env::current_account_id(),
            ACCESS_KEY_METHOD_NAMES.to_string(),
        )
    }

    /// Claim tokens for specific account that are attached to the public key this tx is signed with.
    pub fn claim(&mut self, account_id: AccountId) -> Promise {
        assert_eq!(
            env::predecessor_account_id(),
            env::current_account_id(),
            "Claim only can come from this account"
        );
        assert!(
            env::is_valid_account_id(account_id.as_bytes()),
            "Invalid account id"
        );
        let amount = self
            .accounts
            .remove(&env::signer_account_pk())
            .expect("Unexpected public key");
        Promise::new(env::current_account_id()).delete_key(env::signer_account_pk());
        Promise::new(account_id).transfer(amount)
    }

    /// Create new account and and claim tokens to it.
    pub fn create_account_and_claim(
        &mut self,
        new_account_id: AccountId,
        new_public_key: PublicKey,
    ) -> Promise {
        assert_eq!(
            env::predecessor_account_id(),
            env::current_account_id(),
            "Create account and claim only can come from this account"
        );
        assert!(
            env::is_valid_account_id(new_account_id.as_bytes()),
            "Invalid account id"
        );
        let amount = self
            .accounts
            .remove(&env::signer_account_pk())
            .expect("Unexpected public key");
        Promise::new(new_account_id)
            .create_account()
            .add_full_access_key(new_public_key)
            .transfer(amount)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(ON_CREATE_ACCOUNT_CALLBACK_GAS)
                    .on_account_created_and_claimed(amount),
            )
    }

    /// Create new account without linkdrop and deposit passed funds (used for creating sub accounts directly).
    #[payable]
    pub fn create_account(
        &mut self,
        new_account_id: AccountId,
        new_public_key: PublicKey,
    ) -> Promise {
        assert!(
            env::is_valid_account_id(new_account_id.as_bytes()),
            "Invalid account id"
        );
        let amount = env::attached_deposit();
        Promise::new(new_account_id)
            .create_account()
            .add_full_access_key(new_public_key)
            .transfer(amount)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(ON_CREATE_ACCOUNT_CALLBACK_GAS)
                    .on_account_created(env::predecessor_account_id(), amount),
            )
    }

    /// Create new account without linkdrop and deposit passed funds (used for creating sub accounts directly).
    #[payable]
    pub fn create_account_advanced(
        &mut self,
        new_account_id: AccountId,
        options: CreateAccountOptions,
    ) -> Promise {
        let is_some_option = options.contract_bytes_base64.is_some()
            || options.contract_bytes.is_some()
            || options.full_access_keys.is_some()
            || options.limited_access_keys.is_some()
            || options.use_global_contract_hash.is_some()
            || options.use_global_contract_account_id.is_some();
        assert!(
            is_some_option,
            "Cannot create account with no options. Please specify either contract bytes, full access keys, limited access keys, or global contract options."
        );

        // Count contract deployment options to ensure they're mutually exclusive
        let contract_options_count = [
            options.contract_bytes.is_some(),
            options.contract_bytes_base64.is_some(),
            options.use_global_contract_hash.is_some(),
            options.use_global_contract_account_id.is_some(),
        ].iter().filter(|&&x| x).count();
        
        assert!(
            contract_options_count <= 1,
            "Cannot specify multiple contract deployment options. Choose only one: contract_bytes, contract_bytes_base64, use_global_contract_hash, or use_global_contract_account_id."
        );

        let amount = env::attached_deposit();

        // Initiate a new promise on the new account we're creating and transfer it any attached deposit
        let mut promise = Promise::new(new_account_id)
            .create_account()
            .transfer(amount);

        // If there are any full access keys in the options, loop through and add them to the promise
        if let Some(full_access_keys) = options.full_access_keys {
            for key in full_access_keys {
                promise = promise.add_full_access_key(key.clone());
            }
        }

        // If there are any function call access keys in the options, loop through and add them to the promise
        if let Some(limited_access_keys) = options.limited_access_keys {
            for key_info in limited_access_keys {
                let allowance = if key_info.allowance.as_yoctonear() == 0 {
                    Allowance::Unlimited
                } else {
                    Allowance::Limited(NonZeroU128::new(key_info.allowance.as_yoctonear()).unwrap())
                };
                promise = promise.add_access_key_allowance(
                    key_info.public_key.clone(),
                    allowance,
                    key_info.receiver_id.clone(),
                    key_info.method_names.clone(),
                );
            }
        }

        // If there are any contract bytes, we should deploy the contract to the account
        if let Some(bytes) = options.contract_bytes {
            promise = promise.deploy_contract(bytes);
        };

        // If there are any base 64 contract byte string, we should deploy the contract to the account
        if let Some(bytes) = options.contract_bytes_base64 {
            promise = promise.deploy_contract(bytes.0);
        };

        // If there's a global contract hash, use the existing global contract
        if let Some(code_hash) = options.use_global_contract_hash {
            promise = promise.use_global_contract(code_hash);
        };

        // If there's a global contract account ID, use the existing global contract by account ID
        if let Some(account_id) = options.use_global_contract_account_id {
            promise = promise.use_global_contract_by_account_id(account_id);
        };

        // Callback if anything went wrong, refund the predecessor for their attached deposit
        promise.then(
            Self::ext(env::current_account_id())
                .with_static_gas(ON_CREATE_ACCOUNT_CALLBACK_GAS)
                .on_account_created(env::predecessor_account_id(), amount),
        )
    }

    /// Callback after executing `create_account` or `create_account_advanced`.
    pub fn on_account_created(
        &mut self,
        predecessor_account_id: AccountId,
        amount: NearToken,
    ) -> bool {
        assert_eq!(
            env::predecessor_account_id(),
            env::current_account_id(),
            "Callback can only be called from the contract"
        );
        let creation_succeeded = is_promise_success();
        if !creation_succeeded {
            // In case of failure, send funds back.
            Promise::new(predecessor_account_id).transfer(amount);
        }
        creation_succeeded
    }

    /// Callback after execution `create_account_and_claim`.
    pub fn on_account_created_and_claimed(&mut self, amount: NearToken) -> bool {
        assert_eq!(
            env::predecessor_account_id(),
            env::current_account_id(),
            "Callback can only be called from the contract"
        );
        let creation_succeeded = is_promise_success();
        if creation_succeeded {
            Promise::new(env::current_account_id()).delete_key(env::signer_account_pk());
        } else {
            // In case of failure, put the amount back.
            self.accounts.insert(env::signer_account_pk(), amount);
        }
        creation_succeeded
    }

    /// Returns the balance associated with given key.
    pub fn get_key_balance(&self, key: PublicKey) -> NearToken {
        *self.accounts.get(&key).expect("Key is missing")
    }

    /// Returns information associated with a given key.
    /// Part of the linkdrop NEP
    #[handle_result]
    pub fn get_key_information(&self, key: PublicKey) -> Result<KeyInfo, &'static str> {
        match self.accounts.get(&key) {
            Some(balance) => Ok(KeyInfo { balance: *balance }),
            None => Err("Key is missing"),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;

    use super::*;

    fn linkdrop() -> AccountId {
        "linkdrop".parse().unwrap()
    }

    fn bob() -> AccountId {
        "bob".parse().unwrap()
    }

    #[test]
    fn test_create_account() {
        // Create a new instance of the linkdrop contract
        let mut contract = LinkDrop::new();
        // Create the public key to be used in the test
        let pk: PublicKey = "qSq3LoufLvTCTNGC3LJePMDGrok8dHMQ5A1YD9psbiz"
            .parse()
            .unwrap();
        // Default the deposit to an extremely small amount
        let deposit = NearToken::from_yoctonear(1_000_000);

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .attached_deposit(deposit)
                .context
                .clone()
        );

        // Create bob's account with the PK
        contract.create_account(bob(), pk);
    }

    #[test]
    #[should_panic]
    fn test_create_invalid_account() {
        // Create a new instance of the linkdrop contract
        let mut contract = LinkDrop::new();
        // Create the public key to be used in the test
        let pk: PublicKey = "qSq3LoufLvTCTNGC3LJePMDGrok8dHMQ5A1YD9psbiz"
            .parse()
            .unwrap();
        // Default the deposit to an extremely small amount
        let deposit = NearToken::from_yoctonear(1_000_000);

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .attached_deposit(deposit)
                .context
                .clone()
        );

        // Attempt to create an invalid account with the PK
        contract.create_account("XYZ".parse().unwrap(), pk);
    }

    #[test]
    #[should_panic]
    fn test_get_missing_balance_panics() {
        // Create a new instance of the linkdrop contract
        let contract = LinkDrop::new();
        // Create the public key to be used in the test
        let pk: PublicKey = "qSq3LoufLvTCTNGC3LJePMDGrok8dHMQ5A1YD9psbiz"
            .parse()
            .unwrap();

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .context
                .clone()
        );

        contract.get_key_balance(pk);
    }

    #[test]
    fn test_get_missing_balance_success() {
        // Create a new instance of the linkdrop contract
        let mut contract = LinkDrop::new();
        // Create the public key to be used in the test
        let pk: PublicKey = "qSq3LoufLvTCTNGC3LJePMDGrok8dHMQ5A1YD9psbiz"
            .parse()
            .unwrap();
        // Default the deposit to be 100 times the access key allowance
        let deposit = ACCESS_KEY_ALLOWANCE.saturating_mul(100);

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .attached_deposit(deposit)
                .context
                .clone()
        );

        // Create the linkdrop
        contract.send(pk.clone());

        // try getting the balance of the key
        assert_eq!(
            contract.get_key_balance(pk),
            deposit.checked_sub(ACCESS_KEY_ALLOWANCE).unwrap()
        );
    }

    #[test]
    #[should_panic]
    fn test_claim_invalid_account() {
        // Create a new instance of the linkdrop contract
        let mut contract = LinkDrop::new();
        // Create the public key to be used in the test
        let pk: PublicKey = "qSq3LoufLvTCTNGC3LJePMDGrok8dHMQ5A1YD9psbiz"
            .parse()
            .unwrap();
        // Default the deposit to be 100 times the access key allowance
        let deposit = ACCESS_KEY_ALLOWANCE.saturating_mul(100);

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .attached_deposit(deposit)
                .context
                .clone()
        );

        // Create the linkdrop
        contract.send(pk.clone());

        // Now, send new transaction to linkdrop contract and reinitialize the mocked blockchain with new params
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .predecessor_account_id(linkdrop())
                .signer_account_pk(pk)
                .account_balance(deposit)
                .context
                .clone()
        );

        // Create the second public key
        let pk2 = "2S87aQ1PM9o6eBcEXnTR5yBAVRTiNmvj8J8ngZ6FzSca"
            .parse()
            .unwrap();
        // Attempt to create the account and claim
        contract.create_account_and_claim("XYZ".parse().unwrap(), pk2);
    }

    #[test]
    fn test_drop_claim() {
        // Create a new instance of the linkdrop contract
        let mut contract = LinkDrop::new();
        // Create the public key to be used in the test
        let pk: PublicKey = "qSq3LoufLvTCTNGC3LJePMDGrok8dHMQ5A1YD9psbiz"
            .parse()
            .unwrap();
        // Default the deposit to be 100 times the access key allowance
        let deposit = ACCESS_KEY_ALLOWANCE.saturating_mul(100);

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .attached_deposit(deposit)
                .context
                .clone()
        );

        // Create the linkdrop
        contract.send(pk.clone());

        // Now, send new transaction to linkdrop contract and reinitialize the mocked blockchain with new params
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .predecessor_account_id(linkdrop())
                .signer_account_pk(pk)
                .account_balance(deposit)
                .context
                .clone()
        );

        // Create the second public key
        let pk2 = "2S87aQ1PM9o6eBcEXnTR5yBAVRTiNmvj8J8ngZ6FzSca"
            .parse()
            .unwrap();
        // Attempt to create the account and claim
        contract.create_account_and_claim(bob(), pk2);
    }

    #[test]
    fn test_send_two_times() {
        // Create a new instance of the linkdrop contract
        let mut contract = LinkDrop::new();
        // Create the public key to be used in the test
        let pk: PublicKey = "qSq3LoufLvTCTNGC3LJePMDGrok8dHMQ5A1YD9psbiz"
            .parse()
            .unwrap();
        // Default the deposit to be 100 times the access key allowance
        let deposit = ACCESS_KEY_ALLOWANCE.saturating_mul(100);

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .attached_deposit(deposit)
                .context
                .clone()
        );

        // Create the linkdrop
        contract.send(pk.clone());
        assert_eq!(
            contract.get_key_balance(pk.clone()),
            deposit.checked_sub(ACCESS_KEY_ALLOWANCE).unwrap()
        );

        // Re-initialize the mocked blockchain with new params
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .account_balance(deposit)
                .attached_deposit(deposit.saturating_add(NearToken::from_yoctonear(1)))
                .context
                .clone()
        );

        // Attempt to recreate the same linkdrop twice
        contract.send(pk.clone());
        assert_eq!(
            *contract.accounts.get(&pk).unwrap(),
            NearToken::from_yoctonear(
                deposit.as_yoctonear() + deposit.as_yoctonear() + 1
                    - 2 * ACCESS_KEY_ALLOWANCE.as_yoctonear()
            )
        );
    }

    #[test]
    fn test_create_advanced_account() {
        // Create a new instance of the linkdrop contract
        let mut contract = LinkDrop::new();
        // Create the public key to be used in the test
        let pk: PublicKey = "qSq3LoufLvTCTNGC3LJePMDGrok8dHMQ5A1YD9psbiz"
            .parse()
            .unwrap();
        // Default the deposit to an extremely small amount
        let deposit = NearToken::from_yoctonear(1_000_000);

        // Create options for the advanced account creation
        let options: CreateAccountOptions = CreateAccountOptions {
            full_access_keys: Some(vec![pk.clone()]),
            limited_access_keys: Some(vec![LimitedAccessKey {
                public_key: pk.clone(),
                allowance: NearToken::from_yoctonear(100),
                receiver_id: linkdrop(),
                method_names: "send".to_string(),
            }]),
            contract_bytes: Some(include_bytes!("../target/near/linkdrop.wasm").to_vec()),
            contract_bytes_base64: None,
            global_contract_code: None,
            global_contract_code_by_account_id: None,
            use_global_contract_hash: None,
            use_global_contract_account_id: None,
        };

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .attached_deposit(deposit)
                .context
                .clone()
        );

        // Create bob's account with the advanced options
        contract.create_account_advanced(bob(), options);
    }

    #[test]
    fn test_create_advanced_account_with_base64_contract_byte_string() {
        // Create a new instance of the linkdrop contract
        let mut contract = LinkDrop::new();

        // Default the deposit to an extremely small amount
        let deposit = NearToken::from_yoctonear(1_000_000);

        // Create options for the advanced account creation
        let options: CreateAccountOptions = CreateAccountOptions {
            full_access_keys: None,
            limited_access_keys: None,
            contract_bytes: None,
            contract_bytes_base64: Some(
                include_bytes!("../target/near/linkdrop.wasm")
                    .to_vec()
                    .into(),
            ),
            global_contract_code: None,
            global_contract_code_by_account_id: None,
            use_global_contract_hash: None,
            use_global_contract_account_id: None,
        };

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .attached_deposit(deposit)
                .context
                .clone()
        );

        // Create bob's account with the advanced options
        contract.create_account_advanced(bob(), options);
    }

    #[test]
    #[should_panic]
    fn test_create_advanced_account_no_options() {
        // Create a new instance of the linkdrop contract
        let mut contract = LinkDrop::new();
        // Default the deposit to an extremely small amount
        let deposit = NearToken::from_yoctonear(1_000_000);

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .attached_deposit(deposit)
                .context
                .clone()
        );

        // Create bob's account with the advanced options
        contract.create_account_advanced(
            bob(),
            CreateAccountOptions {
                full_access_keys: None,
                limited_access_keys: None,
                contract_bytes: None,
                contract_bytes_base64: None,
                global_contract_code: None,
                global_contract_code_by_account_id: None,
                use_global_contract_hash: None,
                use_global_contract_account_id: None,
            },
        );
    }

    #[test]
    #[should_panic]
    fn test_create_advanced_account_conflict_contract_bytes() {
        // Create a new instance of the linkdrop contract
        let mut contract = LinkDrop::new();
        // Default the deposit to an extremely small amount
        let deposit = NearToken::from_yoctonear(1_000_000);

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .attached_deposit(deposit)
                .context
                .clone()
        );

        // Create bob's account with the advanced options
        contract.create_account_advanced(
            bob(),
            CreateAccountOptions {
                full_access_keys: None,
                limited_access_keys: None,
                contract_bytes: Some(include_bytes!("../target/near/linkdrop.wasm").to_vec()),
                contract_bytes_base64: Some(
                    include_bytes!("../target/near/linkdrop.wasm")
                        .to_vec()
                        .into(),
                ),
                global_contract_code: None,
                global_contract_code_by_account_id: None,
                use_global_contract_hash: None,
                use_global_contract_account_id: None,
            },
        );
    }

    #[test]
    fn test_create_advanced_account_with_global_contract_code() {
        // Create a new instance of the linkdrop contract
        let mut contract = LinkDrop::new();

        // Default the deposit to an extremely small amount
        let deposit = NearToken::from_yoctonear(1_000_000);

        // Create options for the advanced account creation with global contract code
        let options: CreateAccountOptions = CreateAccountOptions {
            full_access_keys: None,
            limited_access_keys: None,
            contract_bytes: None,
            contract_bytes_base64: None,
            global_contract_code: Some(include_bytes!("../target/near/linkdrop.wasm").to_vec()),
            global_contract_code_by_account_id: None,
            use_global_contract_hash: None,
            use_global_contract_account_id: None,
        };

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .attached_deposit(deposit)
                .context
                .clone()
        );

        // Create bob's account with the advanced options
        contract.create_account_advanced(bob(), options);
    }

    #[test]
    fn test_create_advanced_account_with_global_contract_code_by_account_id() {
        // Create a new instance of the linkdrop contract
        let mut contract = LinkDrop::new();

        // Default the deposit to an extremely small amount
        let deposit = NearToken::from_yoctonear(1_000_000);

        // Create options for the advanced account creation with global contract code by account ID
        let options: CreateAccountOptions = CreateAccountOptions {
            full_access_keys: None,
            limited_access_keys: None,
            contract_bytes: None,
            contract_bytes_base64: None,
            global_contract_code: None,
            global_contract_code_by_account_id: Some(include_bytes!("../target/near/linkdrop.wasm").to_vec()),
            use_global_contract_hash: None,
            use_global_contract_account_id: None,
        };

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .attached_deposit(deposit)
                .context
                .clone()
        );

        // Create bob's account with the advanced options
        contract.create_account_advanced(bob(), options);
    }

    #[test]
    fn test_create_advanced_account_with_use_global_contract_hash() {
        // Create a new instance of the linkdrop contract
        let mut contract = LinkDrop::new();

        // Default the deposit to an extremely small amount
        let deposit = NearToken::from_yoctonear(1_000_000);

        // Create a 32-byte hash for the global contract
        let code_hash = vec![1u8; 32];

        // Create options for the advanced account creation with global contract hash
        let options: CreateAccountOptions = CreateAccountOptions {
            full_access_keys: None,
            limited_access_keys: None,
            contract_bytes: None,
            contract_bytes_base64: None,
            global_contract_code: None,
            global_contract_code_by_account_id: None,
            use_global_contract_hash: Some(code_hash),
            use_global_contract_account_id: None,
        };

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .attached_deposit(deposit)
                .context
                .clone()
        );

        // Create bob's account with the advanced options
        contract.create_account_advanced(bob(), options);
    }

    #[test]
    fn test_create_advanced_account_with_use_global_contract_account_id() {
        // Create a new instance of the linkdrop contract
        let mut contract = LinkDrop::new();

        // Default the deposit to an extremely small amount
        let deposit = NearToken::from_yoctonear(1_000_000);

        // Create options for the advanced account creation with global contract account ID
        let options: CreateAccountOptions = CreateAccountOptions {
            full_access_keys: None,
            limited_access_keys: None,
            contract_bytes: None,
            contract_bytes_base64: None,
            global_contract_code: None,
            global_contract_code_by_account_id: None,
            use_global_contract_hash: None,
            use_global_contract_account_id: Some("deployer.near".parse().unwrap()),
        };

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .attached_deposit(deposit)
                .context
                .clone()
        );

        // Create bob's account with the advanced options
        contract.create_account_advanced(bob(), options);
    }

    #[test]
    #[should_panic]
    fn test_create_advanced_account_conflict_global_contracts() {
        // Create a new instance of the linkdrop contract
        let mut contract = LinkDrop::new();
        // Default the deposit to an extremely small amount
        let deposit = NearToken::from_yoctonear(1_000_000);

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .attached_deposit(deposit)
                .context
                .clone()
        );

        // Try to use both global contract options, which should fail
        contract.create_account_advanced(
            bob(),
            CreateAccountOptions {
                full_access_keys: None,
                limited_access_keys: None,
                contract_bytes: None,
                contract_bytes_base64: None,
                global_contract_code: Some(include_bytes!("../target/near/linkdrop.wasm").to_vec()),
                global_contract_code_by_account_id: None,
                use_global_contract_hash: Some(vec![1u8; 32]),
                use_global_contract_account_id: None,
            },
        );
    }

    #[test]
    #[should_panic]
    fn test_create_advanced_account_conflict_regular_and_global_contracts() {
        // Create a new instance of the linkdrop contract
        let mut contract = LinkDrop::new();
        // Default the deposit to an extremely small amount
        let deposit = NearToken::from_yoctonear(1_000_000);

        // Initialize the mocked blockchain
        testing_env!(
            VMContextBuilder::new()
                .current_account_id(linkdrop())
                .attached_deposit(deposit)
                .context
                .clone()
        );

        // Try to use both regular contract bytes and global contract, which should fail
        contract.create_account_advanced(
            bob(),
            CreateAccountOptions {
                full_access_keys: None,
                limited_access_keys: None,
                contract_bytes: Some(include_bytes!("../target/near/linkdrop.wasm").to_vec()),
                contract_bytes_base64: None,
                global_contract_code: Some(include_bytes!("../target/near/linkdrop.wasm").to_vec()),
                global_contract_code_by_account_id: None,
                use_global_contract_hash: None,
                use_global_contract_account_id: None,
            },
        );
    }
}
