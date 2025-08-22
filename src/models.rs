use near_sdk::json_types::Base64VecU8;
use near_sdk::near;

use crate::*;

/// Information about a specific public key. Should be returned in the `get_key_information` view method.
/// Part of the linkdrop NEP
#[near(serializers=[json])]
pub struct KeyInfo {
    /// yoctoNEAR$ amount that will be sent to the claiming account (either new or existing)
    /// when the key is successfully used.
    pub balance: NearToken,
}

/// Information about any limited access keys that are being added to the account as part of `create_account_advanced`.
#[near(serializers=[json])]
pub struct LimitedAccessKey {
    /// The public key of the limited access key.
    pub public_key: PublicKey,
    /// The amount of yoctoNEAR$ that can be spent on Gas by this key.
    pub allowance: NearToken,
    /// Which contract should this key be allowed to call.
    pub receiver_id: AccountId,
    /// Which methods should this key be allowed to call.
    pub method_names: String,
}

/// Options for `create_account_advanced`.
#[near(serializers=[json])]
pub struct CreateAccountOptions {
    pub full_access_keys: Option<Vec<PublicKey>>,
    pub limited_access_keys: Option<Vec<LimitedAccessKey>>,
    pub contract_bytes: Option<Vec<u8>>,
    pub contract_bytes_base64: Option<Base64VecU8>,
    /// Deploy a global smart contract using the provided contract code.
    pub global_contract_code: Option<Vec<u8>>,
    /// Deploy a global smart contract, identifiable by the predecessor's account ID.
    pub global_contract_code_by_account_id: Option<Vec<u8>>,
    /// Use an existing global contract by code hash (32-byte hash).
    pub use_global_contract_hash: Option<Vec<u8>>,
    /// Use an existing global contract by referencing the account that deployed it.
    pub use_global_contract_account_id: Option<AccountId>,
}
