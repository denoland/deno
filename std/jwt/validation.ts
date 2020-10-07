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
    if (isExpired(payload.exp as number, 1))
      throw RangeError("the jwt is expired");
  }

  if (!verifyAlgorithm(algorithm, header.alg as string))
    throw new Error("no matching algorithm: " + header.alg);

  // The "crit" (critical) Header Parameter indicates that extensions to this
  // specification and/or [JWA] are being used that MUST be understood and
  // processed. (JWS §4.1.11)
  if ("crit" in header) {
    throw new Error(
      "this implementation doesn't process 'crit' header parameter"
    );
  }

  return { header: header as Header, payload, signature };
}
