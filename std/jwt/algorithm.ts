/*
 * An Unsecured JWT is a JWS using the "alg" Header Parameter value "none" and with
 * the empty string for its JWS Signature value, as defined in the JWA specification;
 * it is an Unsecured JWS with the JWT Claims Set as its JWS Payload. (JWT §6)
 */
export type Algorithm = "none" | "HS256" | "HS512";

export function verify(
  algorithm: Algorithm | Array<Exclude<Algorithm, "none">>,
  jwtAlg: string,
): boolean {
  return Array.isArray(algorithm)
    ? (algorithm as string[]).includes(jwtAlg)
    : algorithm === jwtAlg;
}
