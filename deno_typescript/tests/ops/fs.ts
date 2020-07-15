import { log } from "../util.ts";
import * as dispatch from "./dispatch.ts";

export function read(rid: number, size: number): Uint8Array {
  log("read");
  return dispatch.sendSync("op_read", { rid, size }).ok!;
}
