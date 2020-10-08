import { convertUint8ArrayToBase64url } from "./base64/base64url.ts";
import { decodeString as convertHexToUint8Array } from "../encoding/hex.ts";

export function convertHexToBase64url(input: string): string {
  return convertUint8ArrayToBase64url(convertHexToUint8Array(input));
}

export function convertStringToBase64url(input: string): string {
  return convertUint8ArrayToBase64url(new TextEncoder().encode(input));
}
