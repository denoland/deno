import type { Algorithm } from "./algorithm.ts";
import * as base64url from "../encoding/base64url.ts";
import { encodeToString as convertUint8ArrayToHex } from "../encoding/hex.ts";
import {
  create as createSignature,
  verify as verifySignature,
} from "./signature.ts";
import { verify as verifyAlgorithm } from "./algorithm.ts";
/*
 * The following Claim Names are registered in the IANA "JSON Web Token Claims"
 * registry established by Section 10.1. None of the claims defined below are
 * intended to be mandatory to use or implement in all cases, but rather they
 * provide a starting point for a set of useful, interoperable claims.
 * Applications using JWTs should define which specific claims they use and when
 * they are required or optional. (JWT §4.1)
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

export type Payload = PayloadObject | string | unknown[] | null;

/*
 * The "alg" value is a case-sensitive ASCII string containing a StringOrURI value.
 * This Header Parameter MUST be present and MUST be understood and processed by
 * implementations. (JWS §4.1.1)
 */
export interface Header {
  alg: Algorithm;
  [key: string]: unknown;
}

function isExpired(exp: number, leeway = 0): boolean {
  return exp + leeway < Date.now() / 1000;
}

/*
 * Helper function: setExpiration()
 * returns the number of seconds since January 1, 1970, 00:00:00 UTC
 */
export function setExpiration(exp: number | Date): number {
  return Math.round(
    (exp instanceof Date ? exp.getTime() : Date.now() + exp * 1000) / 1000,
  );
}

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
      index === 2
        ? convertUint8ArrayToHex(uint8Array)
        : JSON.parse(new TextDecoder().decode(uint8Array))
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
  payload: PayloadObject | string;
  signature: string;
};

export function isTokenObject(object: TokenObject): object is TokenObject {
  return (
    typeof object?.signature === "string" &&
    typeof object?.header?.alg === "string" &&
    (typeof object?.payload === "object" && object?.payload?.exp
      ? typeof object.payload.exp === "number"
      : true)
  );
}

export async function verify(
  jwt: string,
  key: string,
  {
    algorithm = "HS512",
  }: {
    algorithm?: Algorithm | Array<Exclude<Algorithm, "none">>;
  } = {},
): Promise<Payload> {
  const obj = decode(jwt) as TokenObject;

  if (!isTokenObject(obj)) {
    throw new Error(`jwt is invalid`);
  }

  if (
    obj?.payload !== null &&
    typeof obj?.payload === "object" &&
    "exp" in obj.payload &&
    isExpired(obj.payload.exp!, 1)
  ) {
    throw RangeError("jwt is expired");
  }

  if (!verifyAlgorithm(algorithm, obj.header.alg)) {
    throw new Error(`algorithms do not match`);
  }

  const { header, payload, signature } = obj;

  /*
   * The "crit" (critical) Header Parameter indicates that extensions to this
   * specification and/or [JWA] are being used that MUST be understood and
   * processed. (JWS §4.1.11)
   */
  if ("crit" in obj.header) {
    throw new Error("implementation does not process 'crit' header parameter");
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

const encoder = new TextEncoder();

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
