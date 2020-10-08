import type { Algorithm } from "./algorithm.ts";
import { convertStringToBase64url } from "./_util.ts";
import { convertBase64urlToUint8Array } from "./base64/base64url.ts";
import { encodeToString as convertUint8ArrayToHex } from "../encoding/hex.ts";
import { validate } from "./validation.ts";
import {
  create as createSignature,
  verify as verifySignature,
} from "./signature.ts";

/*
 * The following Claim Names are registered in the IANA "JSON Web Token Claims"
 * registry established by Section 10.1. None of the claims defined below are
 * intended to be mandatory to use or implement in all cases, but rather they
 * provide a starting point for a set of useful, interoperable claims.
 * Applications using JWTs should define which specific claims they use and when
 * they are required or optional. (JWT §4.1)
 */
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

/*
 * The "alg" value is a case-sensitive ASCII string containing a StringOrURI value.
 * This Header Parameter MUST be present and MUST be understood and processed by
 * implementations. (JWS §4.1.1)
 */
export interface Header {
  alg: Algorithm;
  [key: string]: unknown;
}

/*
 * Helper function: setExpiration()
 * returns the number of seconds since January 1, 1970, 00:00:00 UTC
 */
export function setExpiration(exp: number | Date): number {
  return Math.round(
    (exp instanceof Date ? exp.getTime() : Date.now() + exp * 1000) / 1000
  );
}

export function parse(
  jwt: string
): {
  header: unknown;
  payload: unknown;
  signature: unknown;
} {
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
  return `${convertStringToBase64url(
    JSON.stringify(header)
  )}.${convertStringToBase64url(JSON.stringify(payload))}`;
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
