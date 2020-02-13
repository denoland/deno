import { sendSync } from "./dispatch_json.ts";
import * as dispatch from "./dispatch.ts";

/** Check if a given resource is TTY. */
export function isatty(rid: number): boolean {
  return sendSync(dispatch.OP_ISATTY, { rid });
}

/** Set TTY to be under raw mode or not. */
export function setRaw(rid: number, mode: boolean): void {
  sendSync(dispatch.OP_SET_RAW, {
    rid,
    mode
  });
}
