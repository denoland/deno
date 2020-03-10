import { sendSync } from "./dispatch_json.ts";

interface OpenPluginResponse {
  rid: number;
  ops: {
    [name: string]: number;
  };
}

export function openPlugin(filename: string): OpenPluginResponse {
  return sendSync("op_open_plugin", { filename });
}
