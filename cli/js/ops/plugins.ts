import { core } from "../core.ts";

export function openPlugin(filename: string): number {
  const rid = core.dispatchJson.sendSync("op_open_plugin", { filename });
  return rid;
}
