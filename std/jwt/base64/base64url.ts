import {
  convertBase64ToUint8Array,
  convertUint8ArrayToBase64,
} from "./base64.ts";
import {
  addPaddingToBase64url,
} from "../deps.ts";

function convertBase64urlToBase64(base64url: string): string {
  return addPaddingToBase64url(base64url).replace(/\-/g, "+").replace(
    /_/g,
    "/",
  );
}

function convertBase64ToBase64url(base64: string): string {
  return base64.replace(/=/g, "").replace(/\+/g, "-").replace(/\//g, "_");
}

function convertBase64urlToUint8Array(base64url: string): Uint8Array {
  return convertBase64ToUint8Array(convertBase64urlToBase64(base64url));
}

function convertUint8ArrayToBase64url(uint8Array: Uint8Array): string {
  return convertBase64ToBase64url(convertUint8ArrayToBase64(uint8Array));
}

export {
  convertBase64ToBase64url,
  convertBase64urlToBase64,
  convertBase64urlToUint8Array,
  convertUint8ArrayToBase64url,
};
