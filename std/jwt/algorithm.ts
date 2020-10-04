export type Algorithm = "none" | "HS256" | "HS512";

export function verify(
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