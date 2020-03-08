import { sendSync } from "./dispatch_json.ts";

/** Check if a given resource is TTY. */
export function isatty(rid: number): boolean {
  return sendSync("op_isatty", { rid });
}

/** Set TTY to be under raw mode or not. */
export function setRaw(rid: number, mode: boolean): void {
  sendSync("op_set_raw", {
    rid,
    mode
  });
}
