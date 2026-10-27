/**
 * ```ts
 * import { randomNonce } from "./main.ts";
 *
 * randomNonce();
 * ```
 */
export function formatNonceValue(n: string): string {
  return n;
}

export function randomNonce(): string {
  return crypto.randomUUID();
}
