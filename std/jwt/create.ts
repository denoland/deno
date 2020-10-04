import { Algorithm, convertHexToBase64url, convertStringToBase64url, encrypt, Header, Payload } from "./_util.ts";
// https://www.rfc-editor.org/rfc/rfc7515.html#page-8
// The payload can be any content and need not be a representation of a JSON object

export interface Config {
  key: string;
  payload: Payload | unknown;
  header?: Header;
}

export function createSigningInput(header: Header, payload: Payload | unknown): string {
  return `${
    convertStringToBase64url(
      JSON.stringify(header),
    )
  }.${convertStringToBase64url(JSON.stringify(payload))}`;
}

export async function createSignature(
  alg: Algorithm,
  key: string,
  input: string,
): Promise<string> {
  return convertHexToBase64url(await encrypt(alg, key, input));
}

export async function create({
  key,
  payload,
  header = { alg: "HS512", typ: "JWT" },
}: Config): Promise<string> {
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