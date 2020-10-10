import type { Header, Payload } from "./mod.ts";
import type { Algorithm } from "./algorithm.ts";
import { verify as verifyAlgorithm } from "./algorithm.ts";

/*
 * Note that the payload can be any content and need not be a representation of
 * a JSON object. (JWS §3,3)
 */
type TokenObject = {
  header: Header;
  payload: unknown;
  signature: string;
};

function isTokenObject(obj: any): obj is TokenObject {
  return (
    typeof obj?.signature === "string" &&
    typeof obj?.header?.alg === "string" &&
    (typeof obj?.payload === "object" && obj?.payload?.exp
      ? typeof obj.payload.exp === "number"
      : true)
  );
}

/*
 * The "exp" (expiration time) claim identifies the expiration time on
 * or after which the JWT MUST NOT be accepted for processing.
 * Its value MUST be a number containing a NumericDate value.
 * Implementers MAY provide for some small leeway to account for clock skew.
 * Use of this claim is OPTIONAL. (JWT §4.1.4)
 */
function hasExpClaim(obj: any): obj is { payload: { exp: number } } {
  return typeof obj?.payload?.exp === "number";
}

export function isExpired(exp: number, leeway = 0): boolean {
  return exp + leeway < Date.now() / 1000;
}

export function validate(
  obj: { header: unknown; payload: unknown; signature: unknown },
  algorithm: Algorithm | Array<Exclude<Algorithm, "none">>
): TokenObject {
  if (isTokenObject(obj)) {
    if (hasExpClaim(obj)) {
      if (isExpired(obj.payload.exp, 1)) throw RangeError("the jwt is expired");
    }

    if (!verifyAlgorithm(algorithm, obj.header.alg))
      throw new Error("no matching algorithm: " + obj.header.alg);

    /*
     * The "crit" (critical) Header Parameter indicates that extensions to this
     * specification and/or [JWA] are being used that MUST be understood and
     * processed. (JWS §4.1.11)
     */
    if ("crit" in obj.header) {
      throw new Error(
        "this implementation doesn't process 'crit' header parameter"
      );
    }

    return {
      header: obj.header,
      payload: obj.payload,
      signature: obj.signature,
    };
  } else {
    throw Error("the jwt is invalid");
  }
}
