export { setExpiration } from "./_util.ts";
export type { Algorithm } from "./algorithm.ts";

import { isExpired, convertStringToBase64url, isObject } from "./_util.ts";
import { convertBase64urlToUint8Array } from "./base64/base64url.ts";
import { encodeToString as convertUint8ArrayToHex } from "../encoding/hex.ts";
import { Algorithm, verify as verifyAlgorithm } from "./algorithm.ts";
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

export function isTokenObject(object: {
  header: unknown;
  payload: unknown;
  signature: unknown;
}) {
  return (
    typeof object.signature === "string" &&
    isObject(object.header) &&
    typeof object.header?.alg === "string" &&
    (isObject(object.payload) && object.payload?.exp
      ? typeof object.payload.exp === "number"
      : true)
  );
}

export function parse(jwt: string) {
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
  const { header, payload, signature } = parse(jwt);

  if (!isTokenObject({ header, payload, signature })) {
    throw Error("the jwt is invalid");
  }

  if (isExpired(payload.exp!, 1)) throw RangeError("the jwt is expired");

  if (!verifyAlgorithm(algorithm, header.alg)) {
    throw new Error("no matching algorithm: " + header.alg);
  }

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

  // The "crit" (critical) Header Parameter indicates that extensions to this
  // specification and/or [JWA] are being used that MUST be understood and
  // processed. (JWS §4.1.11)
  if ("crit" in header) {
    throw new Error(
      "the jwt is valid but contains the 'crit' header parameter"
    );
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
