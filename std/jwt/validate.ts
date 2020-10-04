import { Algorithm, encrypt, Header, Payload } from "./_util.ts";
import { isExpired } from "./_util.ts";
import { convertBase64urlToUint8Array } from "./base64/base64url.ts";
import {  encodeToString as convertUint8ArrayToHex } from "../encoding/hex.ts";

type TokenObject = { header: Header; payload: Payload; signature: string };

type Validation = {
  jwt: string;
  key: string;
  algorithm?: Algorithm | Algorithm[];
  critHandlers?: Handlers;
};

export type Handlers = {
  [key: string]: (header: unknown) => unknown;
};

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

// A present 'crit' header parameter indicates that the JWS signature validator
// must understand and process additional claims (JWS §4.1.11)
export function checkHeaderCrit(
  header: Header,
  handlers: Handlers,
): unknown {
  if (!isHeaderCrit(header.crit)) {
    throw new Error("header parameter 'crit' must be an array of non-empty strings");
  }

  const newCrit: unknown[] = [...header.crit]

  header.crit.forEach((str: string) => {
    if (!str || typeof str !== "string") {
      throw new Error("header parameter 'crit' values must be non-empty strings");
    }
    if(reservedWords.has(str)) {
      throw new Error("the 'crit' list contains a non-extension header parameter");
    }
    
    const handler = handlers[str]
    const prop = header[str]
    if (!prop || typeof handler !== "function") {
      throw new Error("critical extension header parameters are not understood");
    }

    newCrit.push(handler(prop))
  })

  return newCrit

}

function isHeaderCrit(crit: unknown): crit is string[] {
  return Array.isArray(crit) && crit.every((str: string) => typeof str === "string" && str.length)
}

export function isTokenObject(object: TokenObject): object is TokenObject {
  return (
    typeof object?.signature === "string" &&
    typeof object?.header?.alg === "string" && 
    typeof object?.payload === "object" &&
    object?.payload?.exp ? typeof object.payload.exp === "number" : true
  )
}

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
  critHandlers={},
  algorithm = "HS512",
}: Validation): Promise<Payload> {

  const { header, payload, signature } = parse(jwt);

  if (isExpired(payload.exp!, 1)) {
    throw RangeError("the jwt is expired");
  }

  if (header.crit) {
    await checkHeaderCrit(header, critHandlers);
  }

  const validAlgorithm = validateAlgorithm(algorithm, header.alg);
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
