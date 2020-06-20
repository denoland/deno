import { core } from "../core.ts";

export function isatty(rid: number): boolean {
  return core.dispatchJson.sendSync("op_isatty", { rid });
}

export function setRaw(rid: number, mode: boolean): void {
  core.dispatchJson.sendSync("op_set_raw", {
    rid,
    mode,
  });
}
