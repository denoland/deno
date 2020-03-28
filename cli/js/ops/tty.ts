import { sendSync } from "./dispatch_json.ts";

export function isatty(rid: number): boolean {
  return sendSync("op_isatty", { rid });
}

export function setRaw(rid: number, mode: boolean): void {
  sendSync("op_set_raw", {
    rid,
    mode,
  });
}
