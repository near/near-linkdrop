# LinkDrop contract

LinkDrop contract allows any user to create a link that their friends can use to claim tokens even if they don't have an account yet.

The way it works:

Sender, that has NEAR:
- Creates a new key pair `(pk1, privkey1)`.
- Calls `linkdrop.send(pk1)` with attached balance of NEAR that they want to send.
- Sends a link to any supported wallet app with `privkey1` as part of URL.

Receiver, that doesn't have NEAR:
- Receives link to the wallet with `privkey1`.
- Wallet creates new key pair for this user (or they generate it via HSM) `(pk2, privkey2)`.
- Enters the `new_account_id` receiver want for their new account.
- Wallet creates a transaction to `linkdrop.create_account_and_claim(new_account_id, pk2)`.
- Contract creates new account with `new_account_id` name and `pk2` as full access key and transfers NEAR that Sender sent.

If Receiver already has account (or Sender wants to get back the money):
- Sign tx with `privkey1` to call `linkdrop.claim()`, which transfers money to signer's account.

## Advanced Account Creation

The contract also supports advanced account creation through the `create_account_advanced` method, which allows creating accounts with:

- Custom access keys (full or limited)
- Contract deployment (regular or global contracts)

### Global Contract Support (NEAR SDK 5.17+)

With NEAR SDK 5.17, the contract now supports global contract deployment and usage through the following options:

- `global_contract_code`: Deploy a new global smart contract using the provided contract code
- `global_contract_code_by_account_id`: Deploy a global smart contract identifiable by the predecessor's account ID
- `use_global_contract_hash`: Use an existing global contract by its code hash (32-byte hash)
- `use_global_contract_account_id`: Use an existing global contract by referencing the account that deployed it

### Example Usage

```javascript
// Deploy a new global contract
await contract.create_account_advanced(
  "newaccount.near",
  {
    global_contract_code: contractBytes
  }
);

// Use an existing global contract by hash
await contract.create_account_advanced(
  "newaccount.near", 
  {
    use_global_contract_hash: hashBytes  // 32-byte hash
  }
);

// Use an existing global contract by deployer account
await contract.create_account_advanced(
  "newaccount.near",
  {
    use_global_contract_account_id: "deployer.near"
  }
);
```

**Note**: Only one contract deployment option can be specified at a time. The options are mutually exclusive.

