import type { Algorithm } from "./algorithm.ts";
import { convertHexToBase64url } from "./_util.ts";
import { HmacSha256 } from "../hash/sha256.ts";
import { HmacSha512 } from "../hash/sha512.ts";

function encrypt(
  alg: Algorithm | "none",
  key: string,
  msg: string,
): string {
  switch (alg) {
    case "none":
      return "";
    case "HS256":
      return new HmacSha256(key).update(msg).toString();
    case "HS512":
      return new HmacSha512(key).update(msg).toString();
    default:
      throw new RangeError(
        `algorithm '${alg}' in header is not supported`,
      );
  }
}

export function create(
  alg: Algorithm,
  key: string,
  input: string,
): string {
  return convertHexToBase64url(encrypt(alg, key, input));
}

export function verify({
  signature,
  key,
  alg,
  signingInput,
}: {
  signature: string;
  key: string;
  alg: Algorithm | "none";
  signingInput: string;
}): boolean {
  return signature === encrypt(alg, key, signingInput);
}
