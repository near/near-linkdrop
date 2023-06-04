use near_sdk::json_types::Base64VecU8;
use near_sdk::serde::{Serialize, Deserialize};

use crate::*;

/// Information about a specific public key. Should be returned in the `get_key_information` view method.
/// Part of the linkdrop NEP
#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct KeyInfo {
    /// yoctoNEAR$ amount that will be sent to the claiming account (either new or existing)
    /// when the key is successfully used.
    pub balance: U128,
}


#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
/// Information about any limited access keys that are being added to the account as part of `create_account_advanced`.
pub struct LimitedAccessKey {
    /// The public key of the limited access key.
    pub public_key: PublicKey,
    /// The amount of yoctoNEAR$ that can be spent on Gas by this key.
    pub allowance: U128,
    /// Which contract should this key be allowed to call.
    pub receiver_id: AccountId,
    /// Which methods should this key be allowed to call.
    pub method_names: String,
}
    
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
/// Options for `create_account_advanced`.
pub struct CreateAccountOptions {
    pub full_access_keys: Option<Vec<PublicKey>>,
    pub limited_access_keys: Option<Vec<LimitedAccessKey>>,
    pub contract_bytes: Option<Vec<u8>>,
    pub contract_bytes_base64: Option<Base64VecU8>
}
