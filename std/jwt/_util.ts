import { convertUint8ArrayToBase64url } from "./base64/base64url.ts";
import { decodeString as convertHexToUint8Array } from "../encoding/hex.ts";
import { HmacSha256 } from "../hash/sha256.ts";
import { HmacSha512 } from "../hash/sha512.ts";

// Helper function: setExpiration()
// returns the number of seconds since January 1, 1970, 00:00:00 UTC
export function setExpiration(exp: number | Date): number {
  return Math.round(
    (exp instanceof Date ? exp.getTime() : Date.now() + exp * 1000) / 1000,
  );
}

export function isExpired(exp: number, leeway = 0): boolean {
  return exp + leeway < Date.now() / 1000;
}

export function convertHexToBase64url(input: string): string {
  return convertUint8ArrayToBase64url(convertHexToUint8Array(input));
}

export function convertStringToBase64url(input: string): string {
  return convertUint8ArrayToBase64url(new TextEncoder().encode(input));
}

export type Algorithm = "none" | "HS256" | "HS512";

export async function encrypt(
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
