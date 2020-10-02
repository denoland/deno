import { encrypt } from "./create.ts";
import type { Algorithm, Header, Payload } from "./create.ts";
import { hasProperty, isExpired, isObject } from "./_util.ts";
import { convertBase64urlToUint8Array } from "./base64/base64url.ts";
import { convertUint8ArrayToHex } from "./deps.ts";

type JwtObject = { header: Header; payload: Payload; signature: string };
type JwtObjectWithUnknownProps = {
  header: unknown;
  payload: unknown;
  signature: unknown;
};
export type Validation = {
  jwt: string;
  key: string;
  algorithm?: Algorithm | Algorithm[];
  critHandlers?: Handlers;
};
export type Handlers = {
  [key: string]: (header: unknown) => unknown;
};

// A present 'crit' header parameter indicates that the JWS signature validator
// must understand and process additional claims (JWS §4.1.11)
export function checkHeaderCrit(
  header: Header,
  handlers?: Handlers,
): void {
  const reservedWords = new Set([
    "alg",
    "jku",
    "jwk",
    "kid",
    "x5u",
    "x5c",
    "x5t",
    "x5t#S256",
    "typ",
    "cty",
    "crit",
    "enc",
    "zip",
    "epk",
    "apu",
    "apv",
    "iv",
    "tag",
    "p2s",
    "p2c",
  ]);
  if (!header["crit"]) { return }
  if (
    !Array.isArray(header.crit) ||
    header.crit.some((str: string) => typeof str !== "string" || !str)
  ) {
    throw new Error(
      "header parameter 'crit' must be an array of non-empty strings",
    );
  }
  if (header.crit.some((str: string) => reservedWords.has(str))) {
    throw new Error(
      "the 'crit' list contains a non-extension header parameter",
    );
  }
  if (
    header.crit.some(
      (str: string) =>
        typeof header[str] === "undefined" ||
        typeof handlers?.[str] !== "function",
    )
  ) {
    throw new Error("critical extension header parameters are not understood");
  }

}

export function validateObject(maybeJwtObject: JwtObjectWithUnknownProps): JwtObject {
  if (typeof maybeJwtObject.signature !== "string") {
    throw ReferenceError("the signature is no string");
  }
  if (
    !(
      isObject(maybeJwtObject.header) &&
      hasProperty("alg", maybeJwtObject.header) &&
      typeof maybeJwtObject.header.alg === "string"
    )
  ) {
    throw ReferenceError("header parameter 'alg' is not a string");
  }
  if (
    isObject(maybeJwtObject.payload) &&
    hasProperty("exp", maybeJwtObject.payload)
  ) {
    if (typeof maybeJwtObject.payload.exp !== "number") {
      throw RangeError("claim 'exp' is not a number");
    } // Implementers MAY provide for some small leeway to account for clock skew (JWT §4.1.4)
    else if (isExpired(maybeJwtObject.payload.exp, 1)) {
      throw RangeError("the jwt is expired");
    }
  }
  return maybeJwtObject as JwtObject;
}

export function parseAndDecode(jwt: string): JwtObjectWithUnknownProps {
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

export function validateAlgorithm(
  algorithm: Algorithm | Algorithm[],
  jwtAlg: Algorithm,
): boolean {
  if (Array.isArray(algorithm)) {
    if (algorithm.length > 1 && algorithm.includes("none")) {
      throw new Error("algorithm 'none' must be used alone");
    } else return algorithm.includes(jwtAlg);
  } else {
    return algorithm === jwtAlg;
  }
}

export async function verifySignature({
  signature,
  key,
  alg,
  signingInput,
}: {
  signature: string;
  key: string;
  alg: Algorithm;
  signingInput: string;
}): Promise<boolean> {
  switch (alg) {
    case "none":
    case "HS256":
    case "HS512": {
      return signature === (await encrypt(alg, key, signingInput));
    }
    default:
      throw new RangeError(`no matching crypto alg in the header: ${alg}`);
  }
}

export async function validate({
  jwt,
  key,
  critHandlers,
  algorithm="HS512",
}: Validation): Promise<Payload> {

  const object = validateObject(parseAndDecode(jwt));
  
  await checkHeaderCrit(object.header, critHandlers)
  
  const validAlgorithm = validateAlgorithm(algorithm, object.header.alg);
  if (!validAlgorithm) { throw new Error("no matching algorithm: " + object.header.alg); }
  const validSignature = await verifySignature({
    signature: object.signature,
    key,
    alg: object.header.alg,
    signingInput: jwt.slice(0, jwt.lastIndexOf(".")),
  });
  if (!validSignature) throw new Error("signatures don't match");
  
  return object.payload;
}
