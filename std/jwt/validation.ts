import type { Header, Payload } from "./mod.ts";
import type { Algorithm } from "./algorithm.ts";
import { verify as verifyAlgorithm } from "./algorithm.ts";
import { isExpired, isObject } from "./_util.ts";

export type TokenObjectUnknown = {
  header: unknown;
  payload: unknown;
  signature: unknown;
};

export type TokenObject = {
  header: Header;
  payload: unknown;
  signature: string;
};

export function validate(
  { header, payload, signature }: TokenObjectUnknown,
  algorithm: Algorithm | Array<Exclude<Algorithm, "none">>
): TokenObject {
  if (
    !(
      typeof signature === "string" &&
      isObject(header) &&
      typeof header.alg === "string" &&
      (isObject(payload) && payload?.exp
        ? typeof payload.exp === "number"
        : true)
    )
  )
    throw Error("the jwt is invalid");
  if (isObject(payload) && "exp" in payload) {
    if (isExpired(payload.exp as number, 1)) {
      throw RangeError("the jwt is expired");
    }
  }
  if ("crit" in header) {
    throw new Error(
      "this implementation doesn't accept 'crit' header parameter"
    );
  }
  if (verifyAlgorithm(algorithm, header.alg as string)) {
    return { header: header as Header, payload, signature };
  } else throw new Error("no matching algorithm: " + header.alg);
}
