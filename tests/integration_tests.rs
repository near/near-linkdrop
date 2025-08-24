use anyhow::Result;
use near_api::near_primitives;
use near_api::{
    Account, AccountId, Contract, NearGas, NearToken, NetworkConfig, RPCEndpoint, Signer, Tokens,
    signer,
};
use near_sandbox::{GenesisAccount, Sandbox};
use serde_json::json;
use std::sync::Arc;

const LINKDROP_WASM: &[u8] = include_bytes!("../target/near/linkdrop.wasm");
const NFT_TUTORIAL_WASM: &[u8] = include_bytes!("../__tests__/ext-wasm/nft-tutorial.wasm");

async fn setup_sandbox() -> Result<(
    Sandbox,
    NetworkConfig,
    AccountId,
    Arc<Signer>,
    AccountId,
    Arc<Signer>,
)> {
    // Start sandbox
    let sandbox = Sandbox::start_sandbox_with_version("2.7.0").await?;
    let network_config = NetworkConfig {
        network_name: "sandbox".to_string(),
        rpc_endpoints: vec![RPCEndpoint::new(sandbox.rpc_addr.parse()?)],
        ..NetworkConfig::testnet()
    };

    // Get genesis account
    let genesis_account = GenesisAccount::default();
    let root_id: AccountId = genesis_account.account_id;
    let root_signer: Arc<Signer> = Signer::new(Signer::from_secret_key(
        genesis_account.private_key.parse()?,
    ))?;

    // Deploy linkdrop contract to root account with init call
    Contract::deploy(root_id.clone())
        .use_code(LINKDROP_WASM.to_vec())
        .with_init_call("new", json!({}))?
        .with_signer(root_signer.clone())
        .send_to(&network_config)
        .await?
        .assert_success();

    // Create test account
    let creator_id: AccountId = format!("creator.{}", root_id).parse()?;
    let creator_key = signer::generate_secret_key()?;
    Account::create_account(creator_id.clone())
        .fund_myself(root_id.clone(), NearToken::from_near(50))
        .public_key(creator_key.public_key())?
        .with_signer(root_signer.clone())
        .send_to(&network_config)
        .await?
        .assert_success();
    let creator_signer = Signer::new(Signer::from_secret_key(creator_key))?;

    Ok((
        sandbox,
        network_config,
        root_id,
        root_signer,
        creator_id,
        creator_signer,
    ))
}

fn generate_key_pairs(num_keys: usize) -> Result<Vec<String>> {
    let mut public_keys = Vec::new();

    for _ in 0..num_keys {
        let secret_key = signer::generate_secret_key()?;
        public_keys.push(secret_key.public_key().to_string());
    }

    Ok(public_keys)
}

fn generate_limited_access_key_data(
    pub_keys: &[String],
    receiver_id: &AccountId,
    method_names: &str,
) -> Vec<serde_json::Value> {
    pub_keys
        .iter()
        .map(|pk| {
            json!({
                "public_key": pk,
                "allowance": "0",
                "receiver_id": receiver_id,
                "method_names": method_names,
            })
        })
        .collect()
}

#[test_log::test(tokio::test)]
async fn test_add_5_different_faks_and_limited_access_keys() -> Result<()> {
    let (_sandbox, network, root_id, _root_signer, creator_id, creator_signer) =
        setup_sandbox().await?;

    let public_keys = generate_key_pairs(10)?;
    let new_account_id: AccountId = format!("test1.{}", root_id).parse()?;

    // Check that the new account doesn't exist yet (will fail if exists)
    Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await
        .expect_err("Account should not exist yet");

    let limited_access_keys = &public_keys[0..5];
    let full_access_keys = &public_keys[5..10];

    let limited_keys_data = generate_limited_access_key_data(
        limited_access_keys,
        &root_id,
        "create_account_advanced,bar",
    );

    // Create account with both types of keys
    assert_eq!(
        Contract(root_id.clone())
            .call_function(
                "create_account_advanced",
                json!({
                    "new_account_id": new_account_id,
                    "options": {
                        "limited_access_keys": limited_keys_data,
                        "full_access_keys": full_access_keys.to_vec(),
                    }
                }),
            )?
            .transaction()
            .deposit(NearToken::from_near(2))
            .gas(NearGas::from_tgas(30))
            .with_signer(creator_id, creator_signer)
            .send_to(&network)
            .await?
            .status,
        near_primitives::views::FinalExecutionStatus::SuccessValue(b"true".to_vec()),
        "Account creation with a mix of limited and full access keys must succeed"
    );

    // Verify the new account exists
    let account_balance = Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await
        .expect("Account should exist now");

    // Check balance
    assert!(
        account_balance.total <= NearToken::from_near(2),
        "Balance should be <= 2 NEAR"
    );
    assert!(
        account_balance.total >= NearToken::from_millinear(1900),
        "Balance should be >= 1.9 NEAR"
    );

    Ok(())
}

#[test_log::test(tokio::test)]
async fn test_deploy_nft_contract_without_keys() -> Result<()> {
    let (_sandbox, network, root_id, _root_signer, creator_id, creator_signer) =
        setup_sandbox().await?;

    let new_account_id: AccountId = format!("test2.{}", root_id).parse()?;

    // Check that the new account doesn't exist yet
    Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await
        .expect_err("Account should not exist yet");

    // Deploy NFT tutorial contract (same as original TypeScript test)
    let contract_bytes: Vec<u8> = NFT_TUTORIAL_WASM.to_vec();

    // Create account with NFT contract
    Contract(root_id.clone())
        .call_function(
            "create_account_advanced",
            json!({
                "new_account_id": new_account_id,
                "options": {
                    "contract_bytes": contract_bytes,
                }
            }),
        )?
        .transaction()
        .deposit(NearToken::from_near(10))
        .gas(NearGas::from_tgas(250))
        .with_signer(creator_id.clone(), creator_signer.clone())
        .send_to(&network)
        .await?
        .assert_success();

    // Verify the new account exists
    Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await
        .expect("Account should exist now");

    // Initialize the NFT contract with default metadata
    Contract(new_account_id.clone())
        .call_function(
            "new_default_meta",
            json!({
                "owner_id": creator_id,
            }),
        )?
        .transaction()
        .with_signer(creator_id, creator_signer)
        .send_to(&network)
        .await?
        .assert_success();

    // Verify NFT metadata matches expected values (including GOTEAM symbol)
    let metadata = Contract(new_account_id.clone())
        .call_function("nft_metadata", json!({}))?
        .read_only()
        .fetch_from(&network)
        .await?;

    // Verify the NFT metadata matches expected values
    let metadata_json: &serde_json::Value = &metadata.data;
    assert_eq!(metadata_json["spec"], "nft-1.0.0", "NFT spec should match");
    assert_eq!(
        metadata_json["name"], "NFT Tutorial Contract",
        "NFT name should match"
    );
    assert_eq!(
        metadata_json["symbol"], "GOTEAM",
        "NFT symbol should be GOTEAM"
    );
    assert_eq!(
        metadata_json["icon"],
        serde_json::Value::Null,
        "NFT icon should be null"
    );
    assert_eq!(
        metadata_json["base_uri"],
        serde_json::Value::Null,
        "NFT base_uri should be null"
    );
    assert_eq!(
        metadata_json["reference"],
        serde_json::Value::Null,
        "NFT reference should be null"
    );
    assert_eq!(
        metadata_json["reference_hash"],
        serde_json::Value::Null,
        "NFT reference_hash should be null"
    );

    Ok(())
}

#[test_log::test(tokio::test)]
async fn test_add_2_types_of_access_keys_with_same_public_key() -> Result<()> {
    let (_sandbox, network, root_id, _root_signer, creator_id, creator_signer) =
        setup_sandbox().await?;

    let public_keys = generate_key_pairs(1)?;
    let new_account_id: AccountId = format!("test3.{}", root_id).parse()?;

    // Check that the new account doesn't exist yet
    Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await
        .expect_err("Account should not exist yet");

    let creator_balance_before = Tokens::account(creator_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await?;

    let limited_keys_data =
        generate_limited_access_key_data(&public_keys, &root_id, "create_account_advanced");

    // Try to create account with FAK and limited access key with same public key
    assert_eq!(
        Contract(root_id.clone())
            .call_function(
                "create_account_advanced",
                json!({
                    "new_account_id": new_account_id,
                    "options": {
                        "limited_access_keys": limited_keys_data,
                        "full_access_keys": public_keys,
                    }
                }),
            )?
            .transaction()
            .deposit(NearToken::from_near(2))
            .gas(NearGas::from_tgas(25))
            .with_signer(creator_id.clone(), creator_signer)
            .send_to(&network)
            .await?
            .status,
        near_primitives::views::FinalExecutionStatus::SuccessValue(b"false".to_vec()),
        "Account creation with an overlaping mix of limited and full access keys must fail"
    );

    // Verify the account was not created
    Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await
        .expect_err("Account should not be created");

    // Check that money was refunded to creator
    let creator_balance_after = Tokens::account(creator_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await?;

    // Calculate the difference (should be minimal, just gas fees)
    let balance_diff = creator_balance_before
        .total
        .saturating_sub(creator_balance_after.total);

    // Should only lose gas fees (less than 0.01 NEAR)
    assert!(
        balance_diff < NearToken::from_millinear(10),
        "Creator should get refund minus gas fees"
    );

    Ok(())
}

#[test_log::test(tokio::test)]
async fn test_create_account_with_global_contract_hash() -> Result<()> {
    let (_sandbox, network, root_id, root_signer, creator_id, creator_signer) =
        setup_sandbox().await?;

    let global_contract_account_id: AccountId = format!("global.{}", root_id).parse()?;
    Account::create_account(global_contract_account_id.clone())
        .fund_myself(
            root_id.clone(),
            NearToken::from_near(1)
                .saturating_div(10_000)
                .saturating_mul(NFT_TUTORIAL_WASM.len() as u128 + 30),
        )
        .public_key(root_signer.get_public_key().await?)?
        .with_signer(root_signer.clone())
        .send_to(&network)
        .await?
        .assert_success();

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(NFT_TUTORIAL_WASM);
    let code_hash_bytes = hasher.finalize();
    let code_hash = bs58::encode(&code_hash_bytes).into_string();

    Contract::deploy_global_contract_code(NFT_TUTORIAL_WASM.to_vec())
        .as_hash()
        .with_signer(global_contract_account_id, root_signer)
        .send_to(&network)
        .await?
        .assert_success();

    let new_account_id: AccountId = format!("test_global.{}", root_id).parse()?;

    // Check that the new account doesn't exist yet
    Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await
        .expect_err("Account should not exist yet");

    // Generate a public key for the new account
    let new_account_key = signer::generate_secret_key()?;
    let new_account_public_key = new_account_key.public_key();

    // Create account with global contract hash and a full access key
    assert_eq!(
        Contract(root_id.clone())
            .call_function(
                "create_account_advanced",
                json!({
                    "new_account_id": new_account_id.clone(),
                    "options": {
                        "use_global_contract_hash": code_hash,
                        "full_access_keys": [new_account_public_key.to_string()],
                    }
                }),
            )?
            .transaction()
            // Named accounts on NEAR need to reserve the storage for the account metadata and
            // access keys, so we need to include the deposit for the account. Yet, it does not
            // require to reserve the storage for the contract code if we use global contract, so
            // 0.005 NEAR should be more than enough.
            .deposit(NearToken::from_millinear(5))
            .gas(NearGas::from_tgas(25))
            .with_signer(creator_id, creator_signer)
            .send_to(&network)
            .await?
            .status,
        near_primitives::views::FinalExecutionStatus::SuccessValue(b"true".to_vec()),
        "Account creation with existing global contract hash must succeed"
    );

    // Test 1: Verify the account exists and was created successfully
    let account_balance = Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await
        .expect("Account should exist with global contract");

    // Verify storage usage is minimal (confirming it's using global contract)
    // The storage_usage field shows how much storage the account is using
    // With a global contract, this should be very small (just account data, not contract code)
    assert!(
        account_balance.storage_usage < 300, // Should be just account metadata, not the ~302KB contract code
        "Storage usage should be minimal for global contract. Found: {} bytes. \
         A regular contract deployment would use ~302KB, but global contracts share storage.",
        account_balance.storage_usage
    );

    // Test 2: Verify the contract is actually deployed by interacting with it
    // Initialize the NFT contract
    Contract(new_account_id.clone())
        .call_function(
            "new_default_meta",
            json!({
                "owner_id": new_account_id.to_string()
            }),
        )?
        .transaction()
        .gas(NearGas::from_tgas(30))
        .with_signer(
            new_account_id.clone(),
            Signer::new(Signer::from_secret_key(new_account_key.clone()))?,
        )
        .send_to(&network)
        .await?
        .assert_success();

    // Verify we can call a view method after initialization
    let metadata_result = Contract(new_account_id.clone())
        .call_function("nft_metadata", json!({}))?
        .read_only::<serde_json::Value>()
        .fetch_from(&network)
        .await?;

    // Verify the metadata has expected fields
    assert!(
        metadata_result.data.get("spec").is_some(),
        "NFT metadata should have a spec field"
    );

    Ok(())
}

#[test_log::test(tokio::test)]
async fn test_create_account_with_non_existing_global_contract_hash() -> Result<()> {
    let (_sandbox, network, root_id, _root_signer, creator_id, creator_signer) =
        setup_sandbox().await?;

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(NFT_TUTORIAL_WASM);
    let code_hash_bytes = hasher.finalize();
    let code_hash = bs58::encode(&code_hash_bytes).into_string();

    let new_account_id: AccountId = format!("test_global.{}", root_id).parse()?;

    // Check that the new account doesn't exist yet
    Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await
        .expect_err("Account should not exist yet");

    // Create account with global contract hash
    assert_eq!(
        Contract(root_id.clone())
            .call_function(
                "create_account_advanced",
                json!({
                    "new_account_id": new_account_id,
                    "options": {
                        "use_global_contract_hash": code_hash,
                    }
                }),
            )?
            .transaction()
            .deposit(NearToken::from_near(2))
            .gas(NearGas::from_tgas(25))
            .with_signer(creator_id, creator_signer)
            .send_to(&network)
            .await?
            .status,
        near_primitives::views::FinalExecutionStatus::SuccessValue(b"false".to_vec()),
        "Account creation with non-existing global contract hash must fail"
    );

    Ok(())
}

#[test_log::test(tokio::test)]
async fn test_create_account_with_global_contract_account_id() -> Result<()> {
    let (_sandbox, network, root_id, root_signer, creator_id, creator_signer) =
        setup_sandbox().await?;

    let global_contract_account_id: AccountId = format!("global.{}", root_id).parse()?;
    Account::create_account(global_contract_account_id.clone())
        .fund_myself(
            root_id.clone(),
            NearToken::from_near(1)
                .saturating_div(10_000)
                .saturating_mul(NFT_TUTORIAL_WASM.len() as u128 + 30),
        )
        .public_key(root_signer.get_public_key().await?)?
        .with_signer(root_signer.clone())
        .send_to(&network)
        .await?
        .assert_success();

    Contract::deploy_global_contract_code(NFT_TUTORIAL_WASM.to_vec())
        .as_account_id(global_contract_account_id.clone())
        .with_signer(root_signer)
        .send_to(&network)
        .await?
        .assert_success();

    let new_account_id: AccountId = format!("test_global2.{}", root_id).parse()?;

    // Generate a public key for the new account
    let new_account_key = signer::generate_secret_key()?;
    let new_account_public_key = new_account_key.public_key();

    // Create account with global contract by account ID (referencing the deployer account) and a full access key
    assert_eq!(
        Contract(root_id.clone())
            .call_function(
                "create_account_advanced",
                json!({
                    "new_account_id": new_account_id.clone(),
                    "options": {
                        "use_global_contract_account_id": global_contract_account_id,
                        "full_access_keys": [new_account_public_key.to_string()],
                    }
                }),
            )?
            .transaction()
            // Named accounts on NEAR need to reserve the storage for the account metadata and
            // access keys, so we need to include the deposit for the account. Yet, it does not
            // require to reserve the storage for the contract code if we use global contract, so
            .deposit(NearToken::from_millinear(5))
            .gas(NearGas::from_tgas(25))
            .with_signer(creator_id, creator_signer)
            .send_to(&network)
            .await?
            .status,
        near_primitives::views::FinalExecutionStatus::SuccessValue(b"true".to_vec()),
        "Account creation with existing global contract id must succeed"
    );

    // Test 1: Verify the account exists and was created successfully
    let account_balance = Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await
        .expect("Account should exist with global contract by account ID");

    // Verify storage usage is minimal (confirming it's using global contract)
    // The storage_usage field shows how much storage the account is using
    // With a global contract, this should be very small (just account data, not contract code)
    assert!(
        account_balance.storage_usage < 300, // Should be just account metadata, not the ~302KB contract code
        "Storage usage should be minimal for global contract by account ID. Found: {} bytes. \
         A regular contract deployment would use ~302KB, but global contracts share storage.",
        account_balance.storage_usage
    );

    // Test 2: Verify the contract is actually deployed by interacting with it
    // Initialize the NFT contract
    Contract(new_account_id.clone())
        .call_function(
            "new_default_meta",
            json!({
                "owner_id": new_account_id.to_string()
            }),
        )?
        .transaction()
        .gas(NearGas::from_tgas(30))
        .with_signer(
            new_account_id.clone(),
            Signer::new(Signer::from_secret_key(new_account_key.clone()))?,
        )
        .send_to(&network)
        .await?
        .assert_success();

    // Verify we can call a view method after initialization
    let metadata_result = Contract(new_account_id.clone())
        .call_function("nft_metadata", json!({}))?
        .read_only::<serde_json::Value>()
        .fetch_from(&network)
        .await?;

    // Verify the metadata has expected fields
    assert!(
        metadata_result.data.get("spec").is_some(),
        "NFT metadata should have a spec field"
    );

    Ok(())
}

#[test_log::test(tokio::test)]
async fn test_create_account_with_non_existing_global_contract_account_id() -> Result<()> {
    let (_sandbox, network, root_id, _root_signer, creator_id, creator_signer) =
        setup_sandbox().await?;

    let global_contract_account_id: AccountId = format!("global.{}", root_id).parse()?;

    let new_account_id: AccountId = format!("test_global2.{}", root_id).parse()?;

    // Create account with global contract by account ID (referencing the deployer account)
    assert_eq!(
        Contract(root_id.clone())
            .call_function(
                "create_account_advanced",
                json!({
                    "new_account_id": new_account_id,
                    "options": {
                        "use_global_contract_account_id": global_contract_account_id,
                    }
                }),
            )?
            .transaction()
            .deposit(NearToken::from_near(2))
            .gas(NearGas::from_tgas(25))
            .with_signer(creator_id, creator_signer)
            .send_to(&network)
            .await?
            .status,
        near_primitives::views::FinalExecutionStatus::SuccessValue(b"false".to_vec()),
        "Account creation with non-existing global contract id must fail"
    );

    Ok(())
}
