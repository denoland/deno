import type { Response } from "../types.ts";

export function sendSync(
  _opName: string,
  // eslint-disable-next-line
  _args: Record<string, any>,
): Response {
  return { ok: new Uint8Array() };
}
