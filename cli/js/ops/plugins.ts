import { sendSync } from "./dispatch_json.ts";

export function openPlugin(filename: string): number {
  const rid = sendSync("op_open_plugin", { filename });
  return rid;
}
