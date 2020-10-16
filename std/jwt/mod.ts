import type { Algorithm } from "./algorithm.ts";
import * as base64url from "../encoding/base64url.ts";
import { encodeToString as convertUint8ArrayToHex } from "../encoding/hex.ts";
import {
  create as createSignature,
  verify as verifySignature,
} from "./signature.ts";
import { verify as verifyAlgorithm } from "./algorithm.ts";
/*
 * JWT §4.1: The following Claim Names are registered in the IANA
 * "JSON Web Token Claims" registry established by Section 10.1. None of the
 * claims defined below are intended to be mandatory to use or implement in all
 * cases, but rather they provide a starting point for a set of useful,
 * interoperable claims.
 * Applications using JWTs should define which specific claims they use and when
 * they are required or optional.
 */
export interface PayloadObject {
  iss?: string;
  sub?: string;
  aud?: string[] | string;
  exp?: number;
  nbf?: number;
  iat?: number;
  jti?: string;
  [key: string]: unknown;
}

export type Payload = PayloadObject | string;

/*
 * JWS §4.1.1: The "alg" value is a case-sensitive ASCII string containing a
 * StringOrURI value. This Header Parameter MUST be present and MUST be
 * understood and processed by implementations.
 */
export interface Header {
  alg: Algorithm;
  [key: string]: unknown;
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/*
 * JWT §4.1.4: Implementers MAY provide for some small leeway to account for
 * clock skew.
 */
function isExpired(exp: number, leeway = 0): boolean {
  return exp + leeway < Date.now() / 1000;
}

/*
 * Helper function: setExpiration()
 * returns the number of seconds since January 1, 1970, 00:00:00 UTC
 * @param number in seconds or Date object
 */
export function setExpiration(exp: number | Date): number {
  return Math.round(
    (exp instanceof Date ? exp.getTime() : Date.now() + exp * 1000) / 1000,
  );
}

function tryToParsePayload(input: string): unknown {
  try {
    return JSON.parse(input);
  } catch {
    return input;
  }
}

/*
 * Decodes a jwt into an { header, payload, signature } object
 * @param jwt
 */
export function decode(
  jwt: string,
): {
  header: unknown;
  payload: unknown;
  signature: unknown;
} {
  const parsedArray = jwt
    .split(".")
    .map(base64url.decode)
    .map((uint8Array, index) =>
      index === 0
        ? JSON.parse(decoder.decode(uint8Array))
        : index === 1
        ? tryToParsePayload(decoder.decode(uint8Array))
        : convertUint8ArrayToHex(uint8Array)
    );
  if (parsedArray.length !== 3) throw TypeError("serialization is invalid");

  return {
    header: parsedArray[0],
    payload: parsedArray[1],
    signature: parsedArray[2],
  };
}

export type TokenObject = {
  header: Header;
  payload: unknown;
  signature: string;
};

/*
 * @param object
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function isTokenObject(object: any): object is TokenObject {
  if (
    !(
      typeof object?.signature === "string" &&
      typeof object?.header?.alg === "string"
    )
  ) {
    throw new Error(`jwt is invalid`);
  }
  if (
    typeof object?.payload?.exp === "number" && isExpired(object.payload.exp)
  ) {
    throw RangeError("jwt is expired");
  }
  return true;
}

/*
 * Verify a jwt
 * @param jwt
 * @param key
 * @param object with property 'algorithm'
 */
export async function verify(
  jwt: string,
  key: string,
  {
    algorithm = "HS512",
  }: {
    algorithm?: Algorithm | Array<Exclude<Algorithm, "none">>;
  } = {},
): Promise<unknown> {
  const obj = decode(jwt);

  if (isTokenObject(obj)) {
    if (!verifyAlgorithm(algorithm, obj.header.alg)) {
      throw new Error(`algorithms do not match`);
    }

    const { header, payload, signature } = obj;

    /*
     * JWS §4.1.11: The "crit" (critical) Header Parameter indicates that
     * extensions to this specification and/or [JWA] are being used that MUST be
     * understood and processed.
     */
    if ("crit" in obj.header) {
      throw new Error(
        "implementation does not process 'crit' header parameter",
      );
    }

    if (
      !(await verifySignature({
        signature,
        key,
        alg: header.alg,
        signingInput: jwt.slice(0, jwt.lastIndexOf(".")),
      }))
    ) {
      throw new Error("signatures do not match");
    }

    return payload;
  }
}

/*
 * JSW §7.1: The JWS Compact Serialization represents digitally signed or MACed
 * content as a compact, URL-safe string. This string is:
 *       BASE64URL(UTF8(JWS Protected Header)) || '.' ||
 *       BASE64URL(JWS Payload) || '.' ||
 *       BASE64URL(JWS Signature)
 */
function createSigningInput(header: Header, payload: Payload): string {
  return `${
    base64url.encode(
      encoder.encode(JSON.stringify(header)),
    )
  }.${
    base64url.encode(
      encoder.encode(
        typeof payload === "string" ? payload : JSON.stringify(payload),
      ),
    )
  }`;
}

/*
 * Create a jwt
 * @param payload
 * @param key
 * @param object with property 'header'
 */
export async function create(
  payload: Payload,
  key: string,
  {
    header = { alg: "HS512", typ: "JWT" },
  }: {
    header?: Header;
  } = {},
): Promise<string> {
  const signingInput = createSigningInput(header, payload);
  const signature = await createSignature(header.alg, key, signingInput);
  return `${signingInput}.${signature}`;
}
