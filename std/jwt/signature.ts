import type { Algorithm } from "./algorithm.ts";
import { convertHexToBase64url } from "./_util.ts";
import { HmacSha256 } from "../hash/sha256.ts";
import { HmacSha512 } from "../hash/sha512.ts";

async function encrypt(
  alg: Algorithm,
  key: string,
  msg: string,
): Promise<string> {
  switch (alg) {
    case "none":
      return "";
    case "HS256":
      return new HmacSha256(key).update(msg).toString();
    case "HS512":
      return new HmacSha512(key).update(msg).toString();
    default:
      throw new RangeError(
        `no matching crypto algorithm in the header: ${alg}`,
      );
  }
}

export async function create(
  alg: Algorithm,
  key: string,
  input: string,
): Promise<string> {
  return convertHexToBase64url(await encrypt(alg, key, input));
}

export async function verify({
  signature,
  key,
  alg,
  signingInput,
}: {
  signature: string;
  key: string;
  alg: Algorithm;
  signingInput: string;
}): Promise<boolean> {
  switch (alg) {
    case "none":
    case "HS256":
    case "HS512": {
      return signature === (await encrypt(alg, key, signingInput));
    }
    default:
      throw new RangeError(`no matching crypto alg in the header: ${alg}`);
  }
}
