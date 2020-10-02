import { convertUint8ArrayToBase64url } from "./base64/base64url.ts";
import { convertHexToUint8Array, HmacSha256, HmacSha512, RSA } from "./deps.ts";

// https://www.rfc-editor.org/rfc/rfc7515.html#page-8
// The payload can be any content and need not be a representation of a JSON object
type Payload = PayloadObject | JsonPrimitive | JsonArray;
type Algorithm = "none" | "HS256" | "HS512" | "RS256";
type JsonPrimitive = string | number | boolean | null;
type JsonObject = { [member: string]: JsonValue };
type JsonArray = JsonValue[];
type JsonValue = JsonPrimitive | JsonObject | JsonArray;

interface JwtInput {
  key: string;
  header: Jose;
  payload: Payload;
}

interface PayloadObject {
  iss?: string;
  sub?: string;
  aud?: string[] | string;
  exp?: number;
  nbf?: number;
  iat?: number;
  jti?: string;
  [key: string]: JsonValue | undefined;
}

interface Jose {
  alg: Algorithm;
  crit?: string[];
  [key: string]: JsonValue | undefined;
}

function assertNever(alg: never, message: string): never {
  throw new RangeError(message);
}

// Helper function: setExpiration()
// returns the number of seconds since January 1, 1970, 00:00:00 UTC
function setExpiration(exp: number | Date): number {
  return Math.round(
    (exp instanceof Date ? exp.getTime() : Date.now() + exp * 1000) / 1000,
  );
}

function convertHexToBase64url(input: string): string {
  return convertUint8ArrayToBase64url(convertHexToUint8Array(input));
}

function convertStringToBase64url(input: string): string {
  return convertUint8ArrayToBase64url(new TextEncoder().encode(input));
}

function makeSigningInput(header: Jose, payload: Payload): string {
  return `${
    convertStringToBase64url(
      JSON.stringify(header),
    )
  }.${convertStringToBase64url(JSON.stringify(payload))}`;
}

async function encrypt(
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
    case "RS256":
      return (
        await new RSA(RSA.parseKey(key)).sign(msg, { hash: "sha256" })
      ).hex();
    default:
      assertNever(alg, "no matching crypto algorithm in the header: " + alg);
  }
}

async function makeSignature(
  alg: Algorithm,
  key: string,
  input: string,
): Promise<string> {
  return convertHexToBase64url(await encrypt(alg, key, input));
}

async function makeJwt({ key, header, payload }: JwtInput): Promise<string> {
  try {
    const signingInput = makeSigningInput(header, payload);
    return `${signingInput}.${await makeSignature(
      header.alg,
      key,
      signingInput,
    )}`;
  } catch (err) {
    err.message = `Failed to create JWT: ${err.message}`;
    throw err;
  }
}

export {
  makeJwt,
  encrypt,
  setExpiration,
  makeSignature,
  convertHexToBase64url,
  convertStringToBase64url,
  assertNever,
};

export type { Algorithm, Payload, PayloadObject, Jose, JwtInput, JsonValue };
