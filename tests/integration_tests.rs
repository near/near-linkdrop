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
    let sandbox = Sandbox::start_sandbox().await?;
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
        .await?;

    // Create test account
    let creator_id: AccountId = format!("creator.{}", root_id).parse()?;
    let creator_key = signer::generate_secret_key()?;
    Account::create_account(creator_id.clone())
        .fund_myself(root_id.clone(), NearToken::from_near(50))
        .public_key(creator_key.public_key())?
        .with_signer(root_signer.clone())
        .send_to(&network_config)
        .await?;
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
                "receiver_id": receiver_id.to_string(),
                "method_names": method_names,
            })
        })
        .collect()
}

#[tokio::test]
async fn test_add_5_different_faks_and_limited_access_keys() -> Result<()> {
    let (_sandbox, network, root_id, _root_signer, creator_id, creator_signer) =
        setup_sandbox().await?;

    let public_keys = generate_key_pairs(10)?;
    let new_account_id: AccountId = format!("test1.{}", root_id).parse()?;

    // Check that the new account doesn't exist yet (will fail if exists)
    let account_check = Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await;
    assert!(account_check.is_err(), "Account should not exist yet");

    let limited_access_keys = &public_keys[0..5];
    let full_access_keys = &public_keys[5..10];

    let limited_keys_data = generate_limited_access_key_data(
        limited_access_keys,
        &root_id,
        "create_account_advanced,bar",
    );

    // Create account with both types of keys
    let result = Contract(root_id.clone())
        .call_function(
            "create_account_advanced",
            json!({
                "new_account_id": new_account_id.to_string(),
                "options": {
                    "limited_access_keys": limited_keys_data,
                    "full_access_keys": full_access_keys.to_vec(),
                }
            }),
        )?
        .transaction()
        .deposit(NearToken::from_near(2))
        .gas(NearGas::from_tgas(300))
        .with_signer(creator_id.clone(), creator_signer)
        .send_to(&network)
        .await?;

    // Check that transaction succeeded
    let success = matches!(
        result.transaction_outcome.outcome.status,
        near_primitives::views::ExecutionStatusView::SuccessValue(_)
            | near_primitives::views::ExecutionStatusView::SuccessReceiptId(_)
    );
    assert!(success, "Transaction should succeed");

    // Verify the new account exists
    let account_balance = Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await;
    assert!(account_balance.is_ok(), "Account should exist now");

    // Check balance
    let balance = account_balance.unwrap();
    assert!(
        balance.total <= NearToken::from_near(2),
        "Balance should be <= 2 NEAR"
    );
    assert!(
        balance.total >= NearToken::from_millinear(1900),
        "Balance should be >= 1.9 NEAR"
    );

    Ok(())
}

#[tokio::test]
async fn test_deploy_nft_contract_without_keys() -> Result<()> {
    let (_sandbox, network, root_id, _root_signer, creator_id, creator_signer) =
        setup_sandbox().await?;

    let new_account_id: AccountId = format!("test2.{}", root_id).parse()?;

    // Check that the new account doesn't exist yet
    let account_check = Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await;
    assert!(account_check.is_err(), "Account should not exist yet");

    // Deploy NFT tutorial contract (same as original TypeScript test)
    let contract_bytes: Vec<u8> = NFT_TUTORIAL_WASM.to_vec();

    // Create account with NFT contract
    let result = Contract(root_id.clone())
        .call_function(
            "create_account_advanced",
            json!({
                "new_account_id": new_account_id.to_string(),
                "options": {
                    "contract_bytes": contract_bytes,
                }
            }),
        )?
        .transaction()
        .deposit(NearToken::from_near(10))
        .gas(NearGas::from_tgas(300))
        .with_signer(creator_id.clone(), creator_signer.clone())
        .send_to(&network)
        .await?;

    // Check that transaction succeeded
    let success = matches!(
        result.transaction_outcome.outcome.status,
        near_primitives::views::ExecutionStatusView::SuccessValue(_)
            | near_primitives::views::ExecutionStatusView::SuccessReceiptId(_)
    );
    assert!(success, "Transaction should succeed");

    // Verify the new account exists and has code
    let account_balance = Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await;
    assert!(account_balance.is_ok(), "Account should exist now");

    // Initialize the NFT contract with default metadata
    Contract(new_account_id.clone())
        .call_function(
            "new_default_meta",
            json!({
                "owner_id": creator_id.to_string(),
            }),
        )?
        .transaction()
        .with_signer(creator_id.clone(), creator_signer.clone())
        .send_to(&network)
        .await?;

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

#[tokio::test]
async fn test_add_2_types_of_access_keys_with_same_public_key() -> Result<()> {
    let (_sandbox, network, root_id, _root_signer, creator_id, creator_signer) =
        setup_sandbox().await?;

    let public_keys = generate_key_pairs(1)?;
    let new_account_id: AccountId = format!("test3.{}", root_id).parse()?;

    // Check that the new account doesn't exist yet
    let account_check = Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await;
    assert!(account_check.is_err(), "Account should not exist yet");

    let creator_balance_before = Tokens::account(creator_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await?;

    let limited_keys_data =
        generate_limited_access_key_data(&public_keys, &root_id, "create_account_advanced");

    // Try to create account with FAK and limited access key with same public key
    let result = Contract(root_id.clone())
        .call_function(
            "create_account_advanced",
            json!({
                "new_account_id": new_account_id.to_string(),
                "options": {
                    "limited_access_keys": limited_keys_data,
                    "full_access_keys": public_keys.clone(),
                }
            }),
        )?
        .transaction()
        .deposit(NearToken::from_near(2))
        .with_signer(creator_id.clone(), creator_signer)
        .send_to(&network)
        .await;

    // The transaction will succeed but the account creation will fail in a receipt
    if let Ok(res) = &result {
        // The transaction succeeds but one of the receipts will fail
        let has_add_key_error = res.receipts_outcome.iter().any(|receipt| {
            matches!(
                &receipt.outcome.status,
                near_primitives::views::ExecutionStatusView::Failure(_)
            )
        });
        assert!(
            has_add_key_error,
            "Should have AddKeyAlreadyExists error in receipts"
        );
    } else {
        // Transaction failed at network level, which is also acceptable for this test
        println!("Transaction failed at network level (expected)");
    }

    // Verify the account was not created
    let account_check = Tokens::account(new_account_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await;
    assert!(account_check.is_err(), "Account should not be created");

    // Check that money was refunded to creator
    let creator_balance_after = Tokens::account(creator_id.clone())
        .near_balance()
        .fetch_from(&network)
        .await?;

    // Calculate the difference (should be minimal, just gas fees)
    let balance_diff = if creator_balance_before.total > creator_balance_after.total {
        creator_balance_before.total.as_yoctonear() - creator_balance_after.total.as_yoctonear()
    } else {
        0
    };

    // Should only lose gas fees (less than 0.01 NEAR)
    assert!(
        balance_diff < NearToken::from_millinear(10).as_yoctonear(),
        "Creator should get refund minus gas fees"
    );

    Ok(())
}
