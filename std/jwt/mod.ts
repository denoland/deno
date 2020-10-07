export { setExpiration } from "./_util.ts";

import type { Algorithm } from "./algorithm.ts";
import type { TokenObjectUnknown } from "./validation.ts";
import { convertStringToBase64url, isExpired } from "./_util.ts";
import { convertBase64urlToUint8Array } from "./base64/base64url.ts";
import { encodeToString as convertUint8ArrayToHex } from "../encoding/hex.ts";
import { validate } from "./validation.ts";
import {
  create as createSignature,
  verify as verifySignature,
} from "./signature.ts";

export interface Payload {
  iss?: string;
  sub?: string;
  aud?: Array<string> | string;
  exp?: number;
  nbf?: number;
  iat?: number;
  jti?: string;
  [key: string]: unknown;
}

export interface Header {
  alg: Algorithm;
  [key: string]: unknown;
}

export function parse(jwt: string): TokenObjectUnknown {
  const parsedArray = jwt
    .split(".")
    .map(convertBase64urlToUint8Array)
    .map((uint8Array, index) =>
      index === 2
        ? convertUint8ArrayToHex(uint8Array)
        : JSON.parse(new TextDecoder().decode(uint8Array))
    );
  if (parsedArray.length !== 3) throw TypeError("invalid serialization");

  return {
    header: parsedArray[0],
    payload: parsedArray[1],
    signature: parsedArray[2],
  };
}

export async function verify({
  jwt,
  key,
  algorithm = "HS512",
}: {
  jwt: string;
  key: string;
  algorithm: Algorithm | Array<Exclude<Algorithm, "none">>;
}): Promise<unknown> {
  const { header, payload, signature } = validate(parse(jwt), algorithm);
  if (
    !(await verifySignature({
      signature,
      key,
      alg: header.alg,
      signingInput: jwt.slice(0, jwt.lastIndexOf(".")),
    }))
  ) {
    throw new Error("signatures don't match");
  }

  return payload;
}

function createSigningInput(header: Header, payload: Payload): string {
  return `${
    convertStringToBase64url(
      JSON.stringify(header),
    )
  }.${convertStringToBase64url(JSON.stringify(payload))}`;
}

export async function create({
  key,
  payload,
  header = { alg: "HS512", typ: "JWT" },
}: {
  key: string;
  payload: Payload;
  header?: Header;
}): Promise<string> {
  try {
    const signingInput = createSigningInput(header, payload);
    const signature = await createSignature(header.alg, key, signingInput);

    return `${signingInput}.${signature}`;
  } catch (err) {
    err.message = `Failed to create JWT: ${err.message}`;
    throw err;
  }
}
