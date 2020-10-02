import { encrypt } from "./create.ts";
import type { Algorithm, Header, Payload } from "./create.ts";
import { convertBase64urlToUint8Array } from "./base64/base64url.ts";
import { convertHexToUint8Array, convertUint8ArrayToHex } from "./deps.ts";

type JwtObject = { header: Header; payload: Payload; signature: string };
type JwtObjectWithUnknownProps = {
  header: unknown;
  payload: unknown;
  signature: unknown;
};
type JwtValidation =
  | (JwtObject & { jwt: string; isValid: true; critResult?: unknown[] })
  | { jwt: unknown; error: Error; isValid: false; isExpired: boolean };
type Validation = {
  jwt: string;
  key: string;
  algorithm: Algorithm | Algorithm[];
  critHandlers?: Handlers;
};
type Handlers = {
  [key: string]: (header: unknown) => unknown;
};

function isObject(obj: unknown): obj is object {
  return (
    obj !== null && typeof obj === "object" && Array.isArray(obj) === false
  );
}

function hasProperty<K extends string>(
  key: K,
  x: object,
): x is { [key in K]: unknown } {
  return key in x;
}

function isExpired(exp: number, leeway = 0): boolean {
  return exp + leeway < Date.now() / 1000;
}

// A present 'crit' header parameter indicates that the JWS signature validator
// must understand and process additional claims (JWS §4.1.11)
function checkHeaderCrit(
  header: Header,
  handlers?: Handlers,
): Promise<unknown[]> {
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
  return Promise.all(
    header.crit.map((str: string) => handlers![str](header[str] as unknown)),
  );
}

function validateObject(maybeJwtObject: JwtObjectWithUnknownProps): JwtObject {
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

async function handleJwtObject(
  jwtObject: JwtObject,
  critHandlers?: Handlers,
): Promise<unknown[] | undefined> {
  return jwtObject.header["crit"]
    ? await checkHeaderCrit(jwtObject.header, critHandlers)
    : undefined;
}

function parseAndDecode(jwt: string): JwtObjectWithUnknownProps {
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

function validateAlgorithm(
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

async function verifySignature({
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
      throw new RangeError("no matching crypto alg in the header: " + alg);
  }
}

async function validate({
  jwt,
  key,
  critHandlers,
  algorithm,
}: Validation): Promise<JwtValidation> {
  const object = validateObject(parseAndDecode(jwt));
  const critResult = await handleJwtObject(object, critHandlers);

  const validAlgorithm = validateAlgorithm(algorithm, object.header.alg);
  if (!validAlgorithm) {
    throw new Error("no matching algorithm: " + object.header.alg);
  }
  const validSignature = await verifySignature({
    signature: object.signature,
    key,
    alg: object.header.alg,
    signingInput: jwt.slice(0, jwt.lastIndexOf(".")),
  });
  if (!validSignature) throw new Error("signatures don't match");
  return { ...object, jwt, critResult, isValid: true };
}

export {
  checkHeaderCrit,
  hasProperty,
  isExpired,
  isObject,
  parseAndDecode,
  validate,
  validateObject,
  verifySignature,
};

export type { Handlers, Header, JwtObject, JwtValidation, Payload, Validation };
