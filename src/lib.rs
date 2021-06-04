use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::Map;
use near_sdk::json_types::{Base58PublicKey, U128};
use near_sdk::{
    env, ext_contract, near_bindgen, AccountId, Balance, Promise, PromiseResult, PublicKey,
};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(Default, BorshDeserialize, BorshSerialize)]
pub struct LinkDrop {
    pub accounts: Map<PublicKey, Balance>,
}

/// Access key allowance for linkdrop keys.
const ACCESS_KEY_ALLOWANCE: u128 = 1_000_000_000_000_000_000_000_000;

/// Gas attached to the callback from account creation.
pub const ON_CREATE_ACCOUNT_CALLBACK_GAS: u64 = 20_000_000_000_000;

/// Indicates there are no deposit for a callback for better readability.
const NO_DEPOSIT: u128 = 0;

#[ext_contract(ext_self)]
pub trait ExtLinkDrop {
    /// Callback after plain account creation.
    fn on_account_created(&mut self, predecessor_account_id: AccountId, amount: U128) -> bool;

    /// Callback after creating account and claiming linkdrop.
    fn on_account_created_and_claimed(&mut self, amount: U128) -> bool;
}

fn is_promise_success() -> bool {
    assert_eq!(
        env::promise_results_count(),
        1,
        "Contract expected a result on the callback"
    );
    match env::promise_result(0) {
        PromiseResult::Successful(_) => true,
        _ => false,
    }
}

#[near_bindgen]
impl LinkDrop {
    /// Allows given public key to claim sent balance.
    /// Takes ACCESS_KEY_ALLOWANCE as fee from deposit to cover account creation via an access key.
    #[payable]
    pub fn send(&mut self, public_key: Base58PublicKey) -> Promise {
        assert!(
            env::attached_deposit() > ACCESS_KEY_ALLOWANCE,
            "Attached deposit must be greater than ACCESS_KEY_ALLOWANCE"
        );
        let pk = public_key.into();
        let value = self.accounts.get(&pk).unwrap_or(0);
        self.accounts.insert(
            &pk,
            &(value + env::attached_deposit() - ACCESS_KEY_ALLOWANCE),
        );
        Promise::new(env::current_account_id()).add_access_key(
            pk,
            ACCESS_KEY_ALLOWANCE,
            env::current_account_id(),
            b"claim,create_account_and_claim".to_vec(),
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
        new_public_key: Base58PublicKey,
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
            .add_full_access_key(new_public_key.into())
            .transfer(amount)
            .then(ext_self::on_account_created_and_claimed(
                amount.into(),
                &env::current_account_id(),
                NO_DEPOSIT,
                ON_CREATE_ACCOUNT_CALLBACK_GAS,
            ))
    }

    /// Create new account without linkdrop and deposit passed funds (used for creating sub accounts directly).
    #[payable]
    pub fn create_account(
        &mut self,
        new_account_id: AccountId,
        new_public_key: Base58PublicKey,
    ) -> Promise {
        assert!(
            env::is_valid_account_id(new_account_id.as_bytes()),
            "Invalid account id"
        );
        let amount = env::attached_deposit();
        Promise::new(new_account_id)
            .create_account()
            .add_full_access_key(new_public_key.into())
            .transfer(amount)
            .then(ext_self::on_account_created(
                env::predecessor_account_id(),
                amount.into(),
                &env::current_account_id(),
                NO_DEPOSIT,
                ON_CREATE_ACCOUNT_CALLBACK_GAS,
            ))
    }

    /// Callback after executing `create_account`.
    pub fn on_account_created(&mut self, predecessor_account_id: AccountId, amount: U128) -> bool {
        assert_eq!(
            env::predecessor_account_id(),
            env::current_account_id(),
            "Callback can only be called from the contract"
        );
        let creation_succeeded = is_promise_success();
        if !creation_succeeded {
            // In case of failure, send funds back.
            Promise::new(predecessor_account_id).transfer(amount.into());
        }
        creation_succeeded
    }

    /// Callback after execution `create_account_and_claim`.
    pub fn on_account_created_and_claimed(&mut self, amount: U128) -> bool {
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
            self.accounts
                .insert(&env::signer_account_pk(), &amount.into());
        }
        creation_succeeded
    }

    /// Returns the balance associated with given key.
    pub fn get_key_balance(&self, key: Base58PublicKey) -> U128 {
        self.accounts.get(&key.into()).expect("Key is missing").into()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, BlockHeight, PublicKey, VMContext};

    use super::*;

    pub struct VMContextBuilder {
        context: VMContext,
    }

    impl VMContextBuilder {
        pub fn new() -> Self {
            Self {
                context: VMContext {
                    current_account_id: "".to_string(),
                    signer_account_id: "".to_string(),
                    signer_account_pk: vec![0, 1, 2],
                    predecessor_account_id: "".to_string(),
                    input: vec![],
                    block_index: 0,
                    epoch_height: 0,
                    block_timestamp: 0,
                    account_balance: 0,
                    account_locked_balance: 0,
                    storage_usage: 10u64.pow(6),
                    attached_deposit: 0,
                    prepaid_gas: 10u64.pow(18),
                    random_seed: vec![0, 1, 2],
                    is_view: false,
                    output_data_receivers: vec![],
                },
            }
        }

        pub fn current_account_id(mut self, account_id: AccountId) -> Self {
            self.context.current_account_id = account_id;
            self
        }

        #[allow(dead_code)]
        pub fn signer_account_id(mut self, account_id: AccountId) -> Self {
            self.context.signer_account_id = account_id;
            self
        }

        pub fn predecessor_account_id(mut self, account_id: AccountId) -> Self {
            self.context.predecessor_account_id = account_id;
            self
        }

        #[allow(dead_code)]
        pub fn block_index(mut self, block_index: BlockHeight) -> Self {
            self.context.block_index = block_index;
            self
        }

        pub fn attached_deposit(mut self, amount: Balance) -> Self {
            self.context.attached_deposit = amount;
            self
        }

        pub fn account_balance(mut self, amount: Balance) -> Self {
            self.context.account_balance = amount;
            self
        }

        #[allow(dead_code)]
        pub fn account_locked_balance(mut self, amount: Balance) -> Self {
            self.context.account_locked_balance = amount;
            self
        }

        pub fn signer_account_pk(mut self, pk: PublicKey) -> Self {
            self.context.signer_account_pk = pk;
            self
        }

        pub fn finish(self) -> VMContext {
            self.context
        }
    }

    fn linkdrop() -> String {
        "linkdrop".to_string()
    }

    fn bob() -> String {
        "bob".to_string()
    }

    #[test]
    fn test_create_account() {
        let mut contract = LinkDrop::default();
        let pk: Base58PublicKey = "qSq3LoufLvTCTNGC3LJePMDGrok8dHMQ5A1YD9psbiz"
            .try_into()
            .unwrap();
        let deposit = 1_000_000;
        testing_env!(VMContextBuilder::new()
            .current_account_id(linkdrop())
            .attached_deposit(deposit)
            .finish());
        contract.create_account(bob(), pk);
        // TODO: verify that promise was created with funds for given username.
    }

    #[test]
    #[should_panic]
    fn test_create_invalid_account() {
        let mut contract = LinkDrop::default();
        let pk: Base58PublicKey = "qSq3LoufLvTCTNGC3LJePMDGrok8dHMQ5A1YD9psbiz"
            .try_into()
            .unwrap();
        let deposit = 1_000_000;
        testing_env!(VMContextBuilder::new()
            .current_account_id(linkdrop())
            .attached_deposit(deposit)
            .finish());
        contract.create_account("XYZ".to_string(), pk);
    }

    #[test]
    #[should_panic]
    fn test_get_missing_balance_panics() {
        let contract = LinkDrop::default();
        testing_env!(VMContextBuilder::new()
            .current_account_id(linkdrop())
            .finish());
        contract.get_key_balance("qSq3LoufLvTCTNGC3LJePMDGrok8dHMQ5A1YD9psbiz".try_into().unwrap());
    }

    #[test]
    fn test_get_missing_balance_success() {
        let mut contract = LinkDrop::default();
        let pk: Base58PublicKey = "qSq3LoufLvTCTNGC3LJePMDGrok8dHMQ5A1YD9psbiz"
            .try_into()
            .unwrap();
        let deposit = ACCESS_KEY_ALLOWANCE * 100;
        testing_env!(VMContextBuilder::new()
            .current_account_id(linkdrop())
            .attached_deposit(deposit)
            .finish());
        contract.send(pk.clone());
        // try getting the balance of the key
        let balance:u128 = contract.get_key_balance(pk.try_into().unwrap()).try_into().unwrap();
        assert_eq!(
            balance,
            deposit - ACCESS_KEY_ALLOWANCE
        );
    }

    #[test]
    #[should_panic]
    fn test_claim_invalid_account() {
        let mut contract = LinkDrop::default();
        let pk: Base58PublicKey = "qSq3LoufLvTCTNGC3LJePMDGrok8dHMQ5A1YD9psbiz"
            .try_into()
            .unwrap();
        // Deposit money to linkdrop contract.
        let deposit = ACCESS_KEY_ALLOWANCE * 100;
        testing_env!(VMContextBuilder::new()
            .current_account_id(linkdrop())
            .attached_deposit(deposit)
            .finish());
        contract.send(pk.clone());
        // Now, send new transaction to link drop contract.
        let context = VMContextBuilder::new()
            .current_account_id(linkdrop())
            .predecessor_account_id(linkdrop())
            .signer_account_pk(pk.into())
            .account_balance(deposit)
            .finish();
        testing_env!(context);
        let pk2 = "2S87aQ1PM9o6eBcEXnTR5yBAVRTiNmvj8J8ngZ6FzSca"
            .try_into()
            .unwrap();
        contract.create_account_and_claim("XYZ".to_string(), pk2);
    }

    #[test]
    fn test_drop_claim() {
        let mut contract = LinkDrop::default();
        let pk: Base58PublicKey = "qSq3LoufLvTCTNGC3LJePMDGrok8dHMQ5A1YD9psbiz"
            .try_into()
            .unwrap();
        // Deposit money to linkdrop contract.
        let deposit = ACCESS_KEY_ALLOWANCE * 100;
        testing_env!(VMContextBuilder::new()
            .current_account_id(linkdrop())
            .attached_deposit(deposit)
            .finish());
        contract.send(pk.clone());
        // Now, send new transaction to link drop contract.
        let context = VMContextBuilder::new()
            .current_account_id(linkdrop())
            .predecessor_account_id(linkdrop())
            .signer_account_pk(pk.into())
            .account_balance(deposit)
            .finish();
        testing_env!(context);
        let pk2 = "2S87aQ1PM9o6eBcEXnTR5yBAVRTiNmvj8J8ngZ6FzSca"
            .try_into()
            .unwrap();
        contract.create_account_and_claim(bob(), pk2);
        // TODO: verify that proper promises were created.
    }

    #[test]
    fn test_send_two_times() {
        let mut contract = LinkDrop::default();
        let pk: Base58PublicKey = "qSq3LoufLvTCTNGC3LJePMDGrok8dHMQ5A1YD9psbiz"
            .try_into()
            .unwrap();
        // Deposit money to linkdrop contract.
        let deposit = ACCESS_KEY_ALLOWANCE * 100;
        testing_env!(VMContextBuilder::new()
            .current_account_id(linkdrop())
            .attached_deposit(deposit)
            .finish());
        contract.send(pk.clone());
        assert_eq!(contract.get_key_balance(pk.clone()), (deposit - ACCESS_KEY_ALLOWANCE).into());
        testing_env!(VMContextBuilder::new()
            .current_account_id(linkdrop())
            .account_balance(deposit)
            .attached_deposit(deposit + 1)
            .finish());
        contract.send(pk.clone());
        assert_eq!(
            contract.accounts.get(&pk.into()).unwrap(),
            deposit + deposit + 1 - 2 * ACCESS_KEY_ALLOWANCE
        );
    }
}
