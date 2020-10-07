export type Algorithm = "none" | "HS256" | "HS512";

export function verify(
  algorithm: Algorithm | Array<Exclude<Algorithm, "none">>,
  jwtAlg: string
): boolean {
  return Array.isArray(algorithm)
    ? algorithm.includes(jwtAlg as any)
    : algorithm === jwtAlg;
}
