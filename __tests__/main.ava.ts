import anyTest, { TestFn } from "ava";
import { NEAR, NearAccount, Worker } from "near-workspaces";
import { displayFailureLog, generateKeyPairs, generateLimitedAccessKeyData } from "./utils/general";
import {readFileSync} from 'fs';

const test = anyTest as TestFn<{
    worker: Worker;
    accounts: Record<string, NearAccount>;
  }>;

test.beforeEach(async (t) => {
    // Init the worker and start a Sandbox server
    const worker = await Worker.init();
    console.log("worker inited");

    // Prepare sandbox for tests, create accounts, deploy contracts, etc.
    const root = worker.rootAccount;

    // Deploy the linkdrop contract and initialize it
    await root.deploy(`./res/linkdrop.wasm`);
    await root.call(root, 'new', {});
    console.log(`Deployed root contract and initialized`);

    // // Test users
    const creator = await root.createSubAccount('ali');
    
    const claimer = await root.createSubAccount('claimer');
    console.log(`sub-accounts created`);

    // Save state for test runs
    t.context.worker = worker;
    t.context.accounts = { root, creator, claimer };
});

// If the environment is reused, use test.after to replace test.afterEach
test.afterEach(async t => {
    await t.context.worker.tearDown().catch(error => {
        console.log('Failed to tear down the worker:', error);
    });
});

test('Add 5 different FAKs and Limited Access Keys', async t => {
    const { root, creator } = t.context.accounts;
    const {publicKeys} = await generateKeyPairs(10);
    let newAccount = await root.getAccount(`new-account.${root.accountId}`);
    let doesNewAccountExist = await newAccount.exists();
    t.is(doesNewAccountExist, false);

    const res = await creator.callRaw(
        root, 
        'create_account_advanced', 
        {
            new_account_id: newAccount.accountId, 
            options: {
                limited_access_keys: generateLimitedAccessKeyData(publicKeys.slice(0, 5), root.accountId, 'create_account_advanced'),
                full_access_keys: publicKeys.slice(5, 10),
            }
        },
        {
            attachedDeposit: NEAR.parse("2").toString()
        }
    );

    // Check for any failures
    const errors = displayFailureLog(res);
    t.is(errors.length, 0);

    doesNewAccountExist = await newAccount.exists();
    t.is(doesNewAccountExist, true);

    const newAccountKeys = await root.viewAccessKeys(newAccount.accountId);
    console.log('newAccountKeys: ', newAccountKeys)
    t.is(newAccountKeys.keys.length, 10);
});

test('Deploy a contract and no keys', async t => {
    const { root, creator } = t.context.accounts;
    let newAccount = await root.getAccount(`new-account.${root.accountId}`);
    let doesNewAccountExist = await newAccount.exists();
    t.is(doesNewAccountExist, false);

    const contractBytes = Buffer.from(readFileSync('./__tests__/ext-wasm/nft-tutorial.wasm'));
    const bytes =  Array.from(Uint8Array.from(contractBytes));

    // Try to create an account with a FAK and limited access key (both with same public key)
    const res = await creator.callRaw(
        root, 
        'create_account_advanced', 
        {
            new_account_id: newAccount.accountId, 
            options: {
                contract_bytes: bytes,
            }
        },
        {
            attachedDeposit: NEAR.parse("10").toString(),
            gas: "300000000000000",
        }
    );

    // Check for any failures
    const errors = displayFailureLog(res);
    t.is(errors.length, 0);

    doesNewAccountExist = await newAccount.exists();
    t.is(doesNewAccountExist, true);

    const newAccountKeys = await root.viewAccessKeys(newAccount.accountId);
    console.log('newAccountKeys: ', newAccountKeys)
    t.is(newAccountKeys.keys.length, 0);
    
    const accountInfo = await newAccount.accountView();
    console.log('accountInfo: ', accountInfo);
    t.assert(accountInfo.code_hash != '11111111111111111111111111111111');

    await creator.call(newAccount, 'new_default_meta', {owner_id: creator.accountId});
    const meta: any = await newAccount.view('nft_metadata', {});
    console.log('meta: ', meta)
    t.is(meta.symbol, 'GOTEAM');
});

test('Add 2 types of access keys with same public key', async t => {
    const { root, creator } = t.context.accounts;
    const {publicKeys} = await generateKeyPairs(1);
    let newAccount = await root.getAccount(`new-account.${root.accountId}`);
    let doesNewAccountExist = await newAccount.exists();
    t.is(doesNewAccountExist, false);

    // Try to create an account with a FAK and limited access key (both with same public key)
    const res = await creator.callRaw(
        root, 
        'create_account_advanced', 
        {
            new_account_id: newAccount.accountId, 
            options: {
                limited_access_keys: generateLimitedAccessKeyData(publicKeys, root.accountId, 'create_account_advanced'),
                full_access_keys: publicKeys,
            }
        },
        {
            attachedDeposit: NEAR.parse("2").toString()
        }
    );

    // Check for any failures (should be 1 due to adding a key that already exists)
    const errors = displayFailureLog(res);
    t.is(errors.length, 1);
    t.is(errors[0].hasOwnProperty('AddKeyAlreadyExists'), true);

    doesNewAccountExist = await newAccount.exists();
    t.is(doesNewAccountExist, false);

    const newAccountKeys = await root.viewAccessKeys(newAccount.accountId);
    console.log('newAccountKeys: ', newAccountKeys)
    t.is(newAccountKeys.keys.length, 0);
});