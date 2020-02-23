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

/** Check if running in terminal.
 *
 *       console.log(Deno.isTTY().stdout);
 */
export function isTTY(): { stdin: boolean; stdout: boolean; stderr: boolean } {
  return {
    stdin: isatty(0),
    stdout: isatty(1),
    stderr: isatty(2)
  };
}
