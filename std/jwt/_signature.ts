import type { Algorithm } from "./_algorithm.ts";
import { HmacSha256 } from "../hash/sha256.ts";
import { HmacSha512 } from "../hash/sha512.ts";
import { encode as convertUint8ArrayToBase64url } from "../encoding/base64url.ts";
import { decodeString as convertHexToUint8Array } from "../encoding/hex.ts";

export function convertHexToBase64url(input: string): string {
  return convertUint8ArrayToBase64url(convertHexToUint8Array(input));
}

function encrypt(
  algorithm: Algorithm,
  key: string,
  message: string,
): string {
  switch (algorithm) {
    case "none":
      return "";
    case "HS256":
      return new HmacSha256(key).update(message).toString();
    case "HS512":
      return new HmacSha512(key).update(message).toString();
    default:
      throw new RangeError(
        `The algorithm of '${algorithm}' in the header is not supported.`,
      );
  }
}

/**
 * Create a signature
 * @param algorithm
 * @param key
 * @param input
 */
export async function create(
  algorithm: Algorithm,
  key: string,
  input: string,
): Promise<string> {
  return convertHexToBase64url(await encrypt(algorithm, key, input));
}

/**
 * Verify a signature
 * @param signature
 * @param key
 * @param alg
 * @param signingInput
 */
export async function verify({
  signature,
  key,
  algorithm,
  signingInput,
}: {
  signature: string;
  key: string;
  algorithm: Algorithm;
  signingInput: string;
}): Promise<boolean> {
  return signature === (await encrypt(algorithm, key, signingInput));
}
