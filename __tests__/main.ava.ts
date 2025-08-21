import anyTest, { TestFn } from "ava";
import { NearAccount, parseNEAR, Worker } from "near-workspaces";
import {
  displayFailureLog,
  generateKeyPairs,
  generateLimitedAccessKeyData,
} from "./utils/general";
import { readFileSync } from "fs";
import { isAssertClause } from "typescript";

const test = anyTest as TestFn<{
  worker: Worker;
  accounts: Record<string, NearAccount>;
}>;

test.beforeEach(async (t) => {
  // Init the worker and start a Sandbox server
  const worker = await Worker.init();

  // Prepare sandbox for tests, create accounts, deploy contracts, etc.
  const root = worker.rootAccount;

  // Deploy the linkdrop contract and initialize it
  await root.deploy(`./target/near/linkdrop.wasm`);
  await root.call(root, "new", {});

  // // Test users
  const creator = await root.createSubAccount("creator");
  const claimer = await root.createSubAccount("claimer");

  // Save state for test runs
  t.context.worker = worker;
  t.context.accounts = { root, creator, claimer };
});

// If the environment is reused, use test.after to replace test.afterEach
test.afterEach(async (t) => {
  await t.context.worker.tearDown().catch((error) => {
    console.log("Failed to tear down the worker:", error);
  });
});

test("Add 5 different FAKs and Limited Access Keys", async (t) => {
  const { root, creator } = t.context.accounts;
  const { publicKeys } = await generateKeyPairs(10);

  const newAccount = await root.getAccount(`test1.${root.accountId}`);

  const doesNewAccountExist = await newAccount.exists();
  t.is(doesNewAccountExist, false);

  const limited_access_keys = publicKeys.slice(0, 5);
  const full_access_keys = publicKeys.slice(5, 10);

  const res = await creator.callRaw(
    root,
    "create_account_advanced",
    {
      new_account_id: newAccount.accountId,
      options: {
        limited_access_keys: generateLimitedAccessKeyData(
          limited_access_keys,
          root.accountId,
          "create_account_advanced,bar",
        ),
        full_access_keys: full_access_keys,
      },
    },
    {
      attachedDeposit: BigInt(parseNEAR("2")),
      gas: 300000000000000n,
    },
  );

  // Check for any failures
  const errors = displayFailureLog(res);
  t.is(errors.length, 0);

  // The new account exists
  const doesNewAccountExistNow = await newAccount.exists();
  t.is(doesNewAccountExistNow, true);

  // It has the 10 keys we added
  const newAccountKeys = await root.viewAccessKeys(newAccount.accountId);
  t.is(newAccountKeys.keys.length, 10);

  for (const key of newAccountKeys["keys"]) {
    if (limited_access_keys.includes(key["public_key"])) {
      const expectedPermission = {
        FunctionCall: {
          allowance: null,
          method_names: ["create_account_advanced", "bar"],
          receiver_id: root.accountId,
        },
      };
      t.deepEqual(key["access_key"]["permission"], expectedPermission);
    } else {
      t.is(key["access_key"]["permission"], "FullAccess");
    }
  }

  // The account's balance is approx. 2N
  const newAccountBalance = await newAccount.balance();
  t.true(BigInt(newAccountBalance.available) <= BigInt(parseNEAR("2")));
  t.true(BigInt(newAccountBalance.available) >= BigInt(parseNEAR("1.9")));
});

test("Deploy a contract and no keys", async (t) => {
  const { root, creator } = t.context.accounts;

  // An account that does not exist
  const newAccount = await root.getAccount(`test2.${root.accountId}`);
  const doesNewAccountExist = await newAccount.exists();
  t.is(doesNewAccountExist, false);

  // Get the bytes of a contract
  const contractBytes = Buffer.from(
    readFileSync("./__tests__/ext-wasm/nft-tutorial.wasm"),
  );
  const bytes = Array.from(Uint8Array.from(contractBytes));

  // Try to create an account with a contract
  const res = await creator.callRaw(
    root,
    "create_account_advanced",
    {
      new_account_id: newAccount.accountId,
      options: {
        contract_bytes: bytes,
      },
    },
    {
      attachedDeposit: BigInt(parseNEAR("10")),
      gas: 300000000000000n,
    },
  );

  // There were no failures
  const errors = displayFailureLog(res);
  t.is(errors.length, 0);

  // it exists now
  const doesNewAccountExistNow = await newAccount.exists();
  t.is(doesNewAccountExistNow, true);

  // it has no keys
  const newAccountKeys = await root.viewAccessKeys(newAccount.accountId);
  t.is(newAccountKeys.keys.length, 0);

  // it has a contract
  const accountInfo = await newAccount.accountView();
  t.assert(accountInfo.code_hash != "11111111111111111111111111111111");

  // The account's balance is has approx 10N (minus the contract)
  const newAccountBalance = await newAccount.balance();
  t.true(BigInt(newAccountBalance.total) <= BigInt(parseNEAR("10")));
  t.true(BigInt(newAccountBalance.available) < BigInt(parseNEAR("10")));

  // it has the expected methods in the contract
  await creator.call(newAccount, "new_default_meta", {
    owner_id: creator.accountId,
  });
  const meta: any = await newAccount.view("nft_metadata", {});
  const expectedMeta = {
    spec: "nft-1.0.0",
    name: "NFT Tutorial Contract",
    symbol: "GOTEAM",
    icon: null,
    base_uri: null,
    reference: null,
    reference_hash: null,
  };
  t.deepEqual(meta, expectedMeta);
});

test("Add 2 types of access keys with same public key", async (t) => {
  const { root, creator } = t.context.accounts;
  const { publicKeys } = await generateKeyPairs(1);

  const newAccount = await root.getAccount(`test3.${root.accountId}`);
  const doesNewAccountExist = await newAccount.exists();
  t.is(doesNewAccountExist, false);

  const creatorBalance = await creator.balance();

  // Try to create an account with a FAK and limited access key (both with same public key)
  const res = await creator.callRaw(
    root,
    "create_account_advanced",
    {
      new_account_id: newAccount.accountId,
      options: {
        limited_access_keys: generateLimitedAccessKeyData(
          publicKeys,
          root.accountId,
          "create_account_advanced",
        ),
        full_access_keys: publicKeys,
      },
    },
    {
      attachedDeposit: BigInt(parseNEAR("2")),
    },
  );

  // Check for any failures (should be 1 due to adding a key that already exists)
  const errors = res.result.receipts_outcome.flatMap((receipt) => {
    const errorKind = (receipt.outcome.status as any).Failure?.ActionError
      ?.kind;
    return errorKind ? [errorKind] : [];
  });
  t.is(errors.length, 1);
  t.is(errors[0].hasOwnProperty("AddKeyAlreadyExists"), true);

  // The account was not created
  const doesNewAccountExistNow = await newAccount.exists();
  t.is(doesNewAccountExistNow, false);

  // The money went back to the creator
  const newCreatorBalance = await creator.balance();
  t.true(
    BigInt(newCreatorBalance.available) <= BigInt(creatorBalance.available),
  );
  t.true(
    BigInt(newCreatorBalance.available) >=
      BigInt(creatorBalance.available) - BigInt(parseNEAR("0.01")),
  );
});

test("Create account with global contract by account ID", async (t) => {
  const { root, creator } = t.context.accounts;
  const { publicKeys } = await generateKeyPairs(1);

  const newAccount = await root.getAccount(`test4.${root.accountId}`);
  const doesNewAccountExist = await newAccount.exists();
  t.is(doesNewAccountExist, false);

  // Trying to create an account with a global contract referenced by account ID
  const res = await creator.callRaw(
    root,
    "create_account_advanced",
    {
      new_account_id: newAccount.accountId,
      options: {
        full_access_keys: publicKeys,
        global_contract_account_id: "treasury-web4-global.near",
      },
    },
    {
      attachedDeposit: BigInt(parseNEAR("2")),
      gas: 300000000000000n,
    },
  );

  // Check for any failures
  const errors = displayFailureLog(res);
  t.is(errors.length, 0);

  // The new account exists
  const doesNewAccountExistNow = await newAccount.exists();
  t.is(doesNewAccountExistNow, true);

  // It has the key we added
  const newAccountKeys = await root.viewAccessKeys(newAccount.accountId);
  t.is(newAccountKeys.keys.length, 1);
  t.is(newAccountKeys.keys[0]["access_key"]["permission"], "FullAccess");

  // The account's balance is approx. 2N
  const newAccountBalance = await newAccount.balance();
  t.true(BigInt(newAccountBalance.available) <= BigInt(parseNEAR("2")));
  t.true(BigInt(newAccountBalance.available) >= BigInt(parseNEAR("1.9")));
});

test("Create account with global contract by hash", async (t) => {
  const { root, creator } = t.context.accounts;
  const { publicKeys } = await generateKeyPairs(1);

  const newAccount = await root.getAccount(`test5.${root.accountId}`);
  const doesNewAccountExist = await newAccount.exists();
  t.is(doesNewAccountExist, false);

  // Mock global contract hash (32 bytes)
  const globalHash = Array.from(new Uint8Array(32).fill(0));

  // Try to create an account with a global contract referenced by hash
  const res = await creator.callRaw(
    root,
    "create_account_advanced",
    {
      new_account_id: newAccount.accountId,
      options: {
        full_access_keys: publicKeys,
        global_contract_hash: globalHash,
      },
    },
    {
      attachedDeposit: BigInt(parseNEAR("2")),
      gas: 300000000000000n,
    },
  );

  // Check for any failures
  const errors = displayFailureLog(res);
  t.is(errors.length, 0);

  // The new account exists
  const doesNewAccountExistNow = await newAccount.exists();
  t.is(doesNewAccountExistNow, true);

  // It has the key we added
  const newAccountKeys = await root.viewAccessKeys(newAccount.accountId);
  t.is(newAccountKeys.keys.length, 1);
  t.is(newAccountKeys.keys[0]["access_key"]["permission"], "FullAccess");

  // The account's balance is approx. 2N
  const newAccountBalance = await newAccount.balance();
  t.true(BigInt(newAccountBalance.available) <= BigInt(parseNEAR("2")));
  t.true(BigInt(newAccountBalance.available) >= BigInt(parseNEAR("1.9")));
});

test("Fail to create account with both regular contract and global contract", async (t) => {
  const { root, creator } = t.context.accounts;
  const { publicKeys } = await generateKeyPairs(1);

  const newAccount = await root.getAccount(`test6.${root.accountId}`);
  const doesNewAccountExist = await newAccount.exists();
  t.is(doesNewAccountExist, false);

  // Get some contract bytes
  const contractBytes = Array.from(new Uint8Array([1, 2, 3, 4]));

  // Try to create an account with both regular contract and global contract (should fail)
  const res = await creator.callRaw(
    root,
    "create_account_advanced",
    {
      new_account_id: newAccount.accountId,
      options: {
        full_access_keys: publicKeys,
        contract_bytes: contractBytes,
        global_contract_account_id: "treasury-web4-global.near",
      },
    },
    {
      attachedDeposit: BigInt(parseNEAR("2")),
      gas: 300000000000000n,
    },
  );

  // Check for failures - should have an error
  const errors = displayFailureLog(res);
  t.true(errors.length > 0);

  // The account was not created
  const doesNewAccountExistNow = await newAccount.exists();
  t.is(doesNewAccountExistNow, false);
});

test("Fail to create account with both global contract account ID and hash", async (t) => {
  const { root, creator } = t.context.accounts;
  const { publicKeys } = await generateKeyPairs(1);

  const newAccount = await root.getAccount(`test7.${root.accountId}`);
  const doesNewAccountExist = await newAccount.exists();
  t.is(doesNewAccountExist, false);

  // Mock global contract hash (32 bytes)
  const globalHash = Array.from(new Uint8Array(32).fill(0));

  // Try to create an account with both global contract options (should fail)
  const res = await creator.callRaw(
    root,
    "create_account_advanced",
    {
      new_account_id: newAccount.accountId,
      options: {
        full_access_keys: publicKeys,
        global_contract_account_id: "treasury-web4-global.near",
        global_contract_hash: globalHash,
      },
    },
    {
      attachedDeposit: BigInt(parseNEAR("2")),
      gas: 300000000000000n,
    },
  );

  // Check for failures - should have an error
  const errors = displayFailureLog(res);
  t.true(errors.length > 0);

  // The account was not created
  const doesNewAccountExistNow = await newAccount.exists();
  t.is(doesNewAccountExistNow, false);
});
