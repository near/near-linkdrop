use borsh::{BorshDeserialize, BorshSerialize};

use near_bindgen::{AccountId, PublicKey, Balance, env, near_bindgen, Promise};
use near_bindgen::collections::Map;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(Default, BorshDeserialize, BorshSerialize)]
pub struct LinkDrop {
    pub accounts: Map<PublicKey, Balance>,
}

impl LinkDrop {
    /// Allows given public key to claim sent balance.
    pub fn send(&mut self, public_key: PublicKey) {
        self.accounts.insert(&public_key, &env::attached_deposit());
    }

    /// Claim tokens that are attached to the public key this tx is signed with.
    pub fn claim(&mut self) {
        assert_eq!(env::signer_account_id(), env::current_account_id());
        let amount = self.accounts.remove(&env::signer_account_pk()).expect("Unexpected public key");
        Promise::new(env::current_account_id()).delete_key(env::signer_account_pk());
        Promise::new(env::predecessor_account_id()).transfer(amount);
    }

    /// Create new account and and claim tokens to it.
    pub fn create_account_and_claim(&mut self, new_account_id: AccountId, new_public_key: PublicKey) {
        assert_eq!(env::signer_account_id(), env::current_account_id());
        let amount = self.accounts.remove(&env::signer_account_pk()).expect("Unexpected public key");
        Promise::new(env::current_account_id()).delete_key(env::signer_account_pk());
        Promise::new(new_account_id).create_account().add_full_access_key(new_public_key).transfer(amount);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use near_bindgen::{testing_env, VMContext, BlockHeight, PublicKey};
    use near_bindgen::MockedBlockchain;

    use super::*;

    pub struct VMContextBuilder {
        context: VMContext
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
                    block_timestamp: 0,
                    account_balance: 0,
                    account_locked_balance: 0,
                    storage_usage: 10u64.pow(6),
                    attached_deposit: 0,
                    prepaid_gas: 10u64.pow(18),
                    random_seed: vec![0, 1, 2],
                    is_view: false,
                    output_data_receivers: vec![],
                }
            }
        }

        pub fn current_account_id(mut self, account_id: AccountId) -> Self {
            self.context.current_account_id = account_id;
            self
        }

        pub fn signer_account_id(mut self, account_id: AccountId) -> Self {
            self.context.signer_account_id = account_id;
            self
        }

        pub fn predecessor_account_id(mut self, account_id: AccountId) -> Self {
            self.context.predecessor_account_id = account_id;
            self
        }

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
    fn test_drop_claim() {
        let mut contract = LinkDrop::default();
        let pk = vec![0; 33];
        // Deposit money to linkdrop contract.
        let deposit = 1_000_000;
        testing_env!(VMContextBuilder::new().current_account_id(linkdrop()).attached_deposit(deposit).finish());
        contract.send(pk.clone());
        // Now, send new transaction to link drop contract.
        let context = VMContextBuilder::new().current_account_id(linkdrop()).signer_account_id(linkdrop()).signer_account_pk(pk).account_balance(deposit).finish();
        testing_env!(context);
        contract.create_account_and_claim(bob(), vec![1; 33]);
    }
}
