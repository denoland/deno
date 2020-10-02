import { convertHexToBase64url, convertStringToBase64url, setExpiration } from "./_util.ts";
import { HmacSha256, HmacSha512 } from "./deps.ts";

// https://www.rfc-editor.org/rfc/rfc7515.html#page-8
// The payload can be any content and need not be a representation of a JSON object
export type Payload = PayloadObject | unknown;
export type Algorithm = "none" | "HS256" | "HS512";

export interface Config {
  key: string;
  header: Header;
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
  [key: string]: unknown;
}

export interface Header {
  alg: Algorithm;
  crit?: string[];
  [key: string]: unknown;
}

function makeSigningInput(header: Header, payload: Payload): string {
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
    default:
      throw new RangeError(
        `no matching crypto algorithm in the header: ${alg}`,
      );
  }
}

async function makeSignature(
  alg: Algorithm,
  key: string,
  input: string,
): Promise<string> {
  return convertHexToBase64url(await encrypt(alg, key, input));
}

async function create({ key, header, payload }: Config): Promise<string> {
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
  convertHexToBase64url,
  convertStringToBase64url,
  create,
  encrypt,
  makeSignature,
  setExpiration,
};