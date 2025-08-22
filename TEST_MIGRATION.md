# TypeScript to Rust Test Migration Documentation

## Overview

This document describes the complete migration of the NEAR Linkdrop contract test suite from TypeScript (using `near-workspaces-js`) to Rust (using `near-sandbox` and `near-api`). The migration maintains full test coverage while leveraging Rust's type safety and performance benefits.

## Dependencies

### Previous TypeScript Stack
- `near-workspaces`: JavaScript testing framework for NEAR smart contracts
- `ava`: Test runner
- `near-api-js`: NEAR blockchain interaction library

### New Rust Stack
- `near-sandbox` (0.2.0): Rust library for running a local NEAR sandbox environment
- `near-api` (0.6.1): Rust library for interacting with NEAR blockchain
- `tokio`: Async runtime for Rust
- Standard Rust test framework (`cargo test`)

## Test Architecture

### 1. Unit Tests (`src/lib.rs`)
The existing unit tests remain unchanged, testing contract logic directly without blockchain interaction:
- Account creation validation
- Balance management
- Access key management
- Error handling for invalid operations

### 2. Integration Tests (`tests/integration_tests.rs`)
New Rust integration tests that replace the TypeScript tests, providing end-to-end testing with a real NEAR sandbox:

#### Test Setup
```rust
async fn setup_sandbox() -> Result<(Sandbox, NetworkConfig, AccountId, Arc<Signer>, AccountId, Arc<Signer>)>
```
- Starts a local NEAR sandbox instance
- Deploys the linkdrop contract with initialization
- Creates test accounts with funding
- Returns configured network and signers for test execution

#### Helper Functions
- `generate_key_pairs()`: Generates cryptographic key pairs for testing
- `generate_limited_access_key_data()`: Creates limited access key configurations

## Test Coverage

### Test 1: Adding Mixed Access Keys
**Original TypeScript**: `__tests__/main.ava.ts` - "Add 5 different FAKs and Limited Access Keys"

**Rust Implementation**: `test_add_5_different_faks_and_limited_access_keys()`

**Purpose**: Verifies that accounts can be created with both Full Access Keys (FAKs) and Limited Access Keys simultaneously.

**What it tests**:
1. Account doesn't exist before creation
2. Creation with 5 limited access keys and 5 full access keys
3. Transaction succeeds
4. Account exists after creation
5. Balance is correctly set (approximately 2 NEAR minus gas fees)

**Key differences in Rust**:
- Uses `Contract::deploy().use_code().with_init_call()` for atomic deployment and initialization
- Checks for `SuccessReceiptId` status (not just `SuccessValue`)
- Uses `Tokens::account().near_balance()` for balance verification

### Test 2: NFT Contract Deployment Without Keys
**Original TypeScript**: `__tests__/main.ava.ts` - "Deploy a contract and no keys"

**Rust Implementation**: `test_deploy_nft_contract_without_keys()`

**Purpose**: Tests deploying an NFT contract to a new account without any access keys (useful for DAO-controlled contracts).

**What it tests**:
1. Account doesn't exist initially
2. NFT contract deployment succeeds (using `nft-tutorial.wasm`)
3. Account exists with deployed contract
4. NFT contract can be initialized with `new_default_meta`
5. NFT metadata verification including:
   - Spec: "nft-1.0.0"
   - Name: "NFT Tutorial Contract"
   - Symbol: "GOTEAM"
   - All optional fields (icon, base_uri, reference) are null

**Key differences in Rust**:
- Uses the same `nft-tutorial.wasm` file as the TypeScript test
- Verifies all NFT metadata fields individually
- Maintains complete parity with original test functionality

### Test 3: Duplicate Key Detection
**Original TypeScript**: `__tests__/main.ava.ts` - "Add 2 types of access keys with same public key"

**Rust Implementation**: `test_add_2_types_of_access_keys_with_same_public_key()`

**Purpose**: Ensures the system properly rejects attempts to add the same public key as both a full access key and limited access key.

**What it tests**:
1. Account creation fails when duplicate keys are provided
2. Error is properly reported in receipt outcomes
3. Account is not created
4. Funds are refunded (minus gas fees)

**Key differences in Rust**:
- Checks receipt outcomes for failure status
- Validates refund by comparing balances before and after
- Handles both transaction-level and receipt-level failures

## Key API Differences

### Account Creation
**TypeScript**:
```javascript
await root.createSubAccount("creator");
```

**Rust**:
```rust
Account::create_account(creator_id.clone())
    .fund_myself(root_id.clone(), NearToken::from_near(50))
    .public_key(creator_key.public_key())?
    .with_signer(root_signer.clone())
    .send_to(&network_config)
    .await?;
```

### Contract Deployment
**TypeScript**:
```javascript
await root.deploy(`./target/near/linkdrop.wasm`);
await root.call(root, "new", {});
```

**Rust** (atomic deployment with initialization):
```rust
Contract::deploy(root_id.clone())
    .use_code(LINKDROP_WASM.to_vec())
    .with_init_call("new", json!({}))?
    .with_signer(root_signer.clone())
    .send_to(&network_config)
    .await?;
```

### Contract Function Calls
**TypeScript**:
```javascript
await creator.callRaw(root, "create_account_advanced", args, {
    attachedDeposit: BigInt(parseNEAR("2")),
    gas: 300000000000000n,
});
```

**Rust**:
```rust
Contract(root_id.clone())
    .call_function("create_account_advanced", args)?
    .transaction()
    .deposit(NearToken::from_near(2))
    .gas(NearGas::from_tgas(300))
    .with_signer(creator_id.clone(), creator_signer)
    .send_to(&network)
    .await?;
```

### Balance Queries
**TypeScript**:
```javascript
const balance = await account.balance();
```

**Rust**:
```rust
let balance = Tokens::account(account_id.clone())
    .near_balance()
    .fetch_from(&network)
    .await?;
```

## Migration Benefits

1. **Type Safety**: Rust's type system catches errors at compile time
2. **Performance**: Native execution without JavaScript runtime overhead
3. **Unified Toolchain**: Single language for contract and tests
4. **Better IDE Support**: Full IntelliSense and refactoring capabilities
5. **Dependency Management**: Cargo provides deterministic builds and better dependency resolution

## Running the Tests

### Unit Tests Only
```bash
cargo test --lib
```

### Integration Tests Only
```bash
cargo test --test integration_tests
```

### All Tests
```bash
cargo test
```

### With Output
```bash
cargo test -- --nocapture
```

### Release Mode (Optimized)
```bash
cargo test --release
```

## Files Removed

The following TypeScript test infrastructure was removed:
- `__tests__/main.ava.ts` - Main test file (functionality moved to `tests/integration_tests.rs`)
- `__tests__/utils/general.ts` - Test utilities (functionality integrated into test file)
- `package.json` - Node.js dependencies
- `yarn.lock` / `package-lock.json` - Lock files
- `ava.config.cjs` - Test runner configuration

## Files Retained

- `__tests__/ext-wasm/nft-tutorial.wasm` - NFT contract binary used for testing contract deployment
  - This file is essential for maintaining test parity with the original TypeScript tests
  - Used in `test_deploy_nft_contract_without_keys()` to verify NFT contract deployment and metadata

## Troubleshooting

### Common Issues

1. **Sandbox startup failures**: Ensure no other NEAR sandbox instances are running
2. **Test timeouts**: Integration tests require network communication; increase timeout if needed
3. **Compilation errors**: Run `cargo build` first to ensure WASM contract is built

### Debug Output

To see transaction details during test execution:
```rust
println!("Transaction result: {:?}", result.transaction_outcome.outcome.status);
println!("Receipt outcomes: {:?}", res.receipts_outcome.iter().map(|r| &r.outcome.status).collect::<Vec<_>>());
```

## Future Improvements

1. **Parallel Test Execution**: Tests currently run sequentially; could be optimized for parallel execution
2. **Custom Assertions**: Create test helpers for common NEAR-specific assertions
3. **Property-Based Testing**: Add quickcheck or proptest for fuzzing
4. **Gas Profiling**: Add gas usage tracking and optimization tests
5. **Cross-Contract Testing**: Add tests for interaction with other contracts

## Conclusion

The migration from TypeScript to Rust tests provides a more robust, performant, and maintainable test suite. The tests maintain complete functional parity with the original TypeScript tests while leveraging Rust's strengths. The use of `near-sandbox` and `near-api` provides a native Rust testing experience that integrates seamlessly with the Rust smart contract development workflow.