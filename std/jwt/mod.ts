export { setExpiration } from "./_util.ts"
export type { Payload } from "./_util.ts";
export type { Header } from "./header.ts";
export type { Algorithm } from "./algorithm.ts";

import { createSigningInput, isTokenObject, Payload, TokenObject } from "./_util.ts";
import { isExpired } from "./_util.ts";
import { convertBase64urlToUint8Array } from "./base64/base64url.ts";
import { encodeToString as convertUint8ArrayToHex } from "../encoding/hex.ts";
import { Algorithm, verify as verifyAlgorithm } from "./algorithm.ts"
import { create as createSignature, verify as verifySignature } from "./signature.ts"
import { verifyHeaderCrit, Handlers, Header } from "./header.ts"

export function parse(jwt: string): TokenObject {
  const parsedArray = jwt
    .split(".")
    .map(convertBase64urlToUint8Array)
    .map((uint8Array, index) =>
      index === 2
        ? convertUint8ArrayToHex(uint8Array)
        : JSON.parse(new TextDecoder().decode(uint8Array))
    );
  if (parsedArray.length !== 3) throw TypeError("invalid serialization");

  const object = {
    header: parsedArray[0],
    payload: parsedArray[1],
    signature: parsedArray[2],
  };

  if (!isTokenObject(object)) {
    throw Error("the jwt is invalid");
  }
  
  return object
}

export async function verify({
  jwt,
  key,
  critHandlers={},
  algorithm = "HS512",
}: {
  jwt: string;
  key: string;
  algorithm?: Algorithm | Algorithm[];
  critHandlers?: Handlers;
}): Promise<Payload> {

  const { header, payload, signature } = parse(jwt);

  if (isExpired(payload.exp!, 1)) {
    throw RangeError("the jwt is expired");
  }

  if (header.crit) {
    await verifyHeaderCrit(header, critHandlers);
  }

  const validAlgorithm = verifyAlgorithm(algorithm, header.alg);
  if (!validAlgorithm) {
    throw new Error("no matching algorithm: " + header.alg);
  }
  const validSignature = await verifySignature({
    signature,
    key,
    alg: header.alg,
    signingInput: jwt.slice(0, jwt.lastIndexOf(".")),
  });

  if (!validSignature) throw new Error("signatures don't match");

  return payload;
}

export async function create({
  key,
  payload,
  header = { alg: "HS512", typ: "JWT" },
}: {
  key: string;
  payload: Payload | unknown;
  header?: Header;
}): Promise<string> {
  try {
    const signingInput = createSigningInput(header, payload);
    const signature = await createSignature(
      header.alg,
      key,
      signingInput,
    )
    return `${signingInput}.${signature}`;
  } catch (err) {
    err.message = `Failed to create JWT: ${err.message}`;
    throw err;
  }
}