import { convertUint8ArrayToBase64url } from "./base64/base64url.ts";
import { decodeString as convertHexToUint8Array } from "../encoding/hex.ts";
import { HmacSha256 } from "../hash/sha256.ts";
import { HmacSha512 } from "../hash/sha512.ts";
import type { Algorithm } from "./algorithm.ts"
import type { Header } from "./header.ts"

export type TokenObject = { header: Header; payload: Payload; signature: string };
export interface Payload {
  iss?: string;
  sub?: string;
  aud?: string[] | string;
  exp?: number;
  nbf?: number;
  iat?: number;
  jti?: string;
  [key: string]: unknown;
}

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

export function isTokenObject(object: TokenObject): object is TokenObject {
  return (
    typeof object?.signature === "string" &&
    typeof object?.header?.alg === "string" && 
    typeof object?.payload === "object" &&
    object?.payload?.exp ? typeof object.payload.exp === "number" : true
  )
}

export function createSigningInput(header: Header, payload: Payload | unknown): string {
  return `${
    convertStringToBase64url(
      JSON.stringify(header),
    )
  }.${convertStringToBase64url(JSON.stringify(payload))}`;
}

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
