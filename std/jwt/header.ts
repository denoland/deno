import type { Algorithm } from "./algorithm.ts"

export interface Header {
  alg: Algorithm;
  crit?: string[];
  [key: string]: unknown;
}

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
export function verifyHeaderCrit(
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
