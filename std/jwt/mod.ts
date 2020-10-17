import type { Algorithm, AlgorithmInput } from "./_algorithm.ts";
import * as base64url from "../encoding/base64url.ts";
import { encodeToString as convertUint8ArrayToHex } from "../encoding/hex.ts";
import {
  create as createSignature,
  verify as verifySignature,
} from "./_signature.ts";
import { verify as verifyAlgorithm } from "./_algorithm.ts";

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

function tryToParsePayload(input: string): unknown {
  try {
    return JSON.parse(input);
  } catch {
    return input;
  }
}

/**
 * Decodes a token into an { header, payload, signature } object.
 * @param token
 */
export function decode(
  token: string,
): {
  header: Header;
  payload: unknown;
  signature: string;
} {
  const parsedArray = token
    .split(".")
    .map(base64url.decode)
    .map((uint8Array, index) => {
      switch (index) {
        case 0:
          try {
            return JSON.parse(decoder.decode(uint8Array));
          } catch {
            break;
          }
        case 1:
          return tryToParsePayload(decoder.decode(uint8Array));
        case 2:
          return convertUint8ArrayToHex(uint8Array);
      }
      throw TypeError("The serialization is invalid.");
    });

  const [header, payload, signature] = parsedArray;

  if (
    !(
      (typeof signature === "string" &&
          typeof header?.alg === "string") && payload?.exp !== undefined
        ? typeof payload.exp === "number"
        : true
    )
  ) {
    throw new Error(`The token is invalid.`);
  }

  if (
    typeof payload?.exp === "number" &&
    isExpired(payload.exp)
  ) {
    throw RangeError("The token is expired.");
  }

  return {
    header,
    payload,
    signature,
  };
}

export type VerifyOptions = {
  algorithm?: AlgorithmInput;
};

/**
 * Verifies a token.
 * @param token
 * @param key
 * @param object with property 'algorithm'
 */
export async function verify(
  token: string,
  key: string,
  { algorithm = "HS512" }: VerifyOptions = {},
): Promise<unknown> {
  const { header, payload, signature } = decode(token);

  if (!verifyAlgorithm(algorithm, header.alg)) {
    throw new Error(
      `The token's algorithm does not match the specified algorithm '${algorithm}'.`,
    );
  }

  /*
   * JWS §4.1.11: The "crit" (critical) Header Parameter indicates that
   * extensions to this specification and/or [JWA] are being used that MUST be
   * understood and processed.
   */
  if ("crit" in header) {
    throw new Error(
      "The 'crit' header parameter is currently not supported by this module.",
    );
  }

  if (
    !(await verifySignature({
      signature,
      key,
      algorithm: header.alg,
      signingInput: token.slice(0, token.lastIndexOf(".")),
    }))
  ) {
    throw new Error(
      "The token's signature does not match the verification signature.",
    );
  }

  return payload;
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

/**
 * Creates a token.
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
