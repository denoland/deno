/*
 * JSW §1: Cryptographic algorithms and identifiers for use with this specification
 * are described in the separate JSON Web Algorithms (JWA) specification:
 * https://www.rfc-editor.org/rfc/rfc7518
 */
export type Algorithm = "none" | "HS256" | "HS512";

/*
 * Verify the algorithm
 * @param algorithm as string or multiple algorithms in an array excluding 'none'
 * @param the algorithm from the jwt header
 */
export function verify(
  algorithm: Algorithm | Array<Exclude<Algorithm, "none">>,
  jwtAlg: string,
): boolean {
  return Array.isArray(algorithm)
    ? (algorithm as string[]).includes(jwtAlg)
    : algorithm === jwtAlg;
}
