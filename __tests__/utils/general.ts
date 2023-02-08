import { KeyPair } from "near-api-js";
import { TransactionResult } from "near-workspaces";

export function generateLimitedAccessKeyData(
  pubKeys: string[],
  receiverId: string,
  methodName: string
) {
  let keys: any = []
  for (let i = 0; i < pubKeys.length; i++) {
    keys.push({
      public_key: pubKeys[i],
      allowance: "0",
      receiver_id: receiverId,
      method_names: methodName
    })
  }

  return keys
}

export async function generateKeyPairs(
    numKeys: number,
  ): Promise<{ keys: KeyPair[]; publicKeys: string[] }> {
    // Generate NumKeys public keys
    let kps: KeyPair[] = [];
    let pks: string[] = [];
    for (let i = 0; i < numKeys; i++) {
      let keyPair = await KeyPair.fromRandom('ed25519');
      kps.push(keyPair);
      pks.push(keyPair.getPublicKey().toString());
    }
    return {
      keys: kps,
      publicKeys: pks
    }
}

export function displayFailureLog(
  transaction: TransactionResult
) {
  let errors: any = [];
  // Loop through each receipts_outcome in the transaction's result field
  transaction.result.receipts_outcome.forEach((receipt) => {
    const status = (receipt.outcome.status as any);
    if (status.Failure?.ActionError?.kind) {
      errors.push(status.Failure?.ActionError?.kind);
      console.log('Failure: ', status.Failure?.ActionError?.kind)
    }
  })

  return errors;
}