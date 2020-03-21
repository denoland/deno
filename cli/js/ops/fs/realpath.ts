// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export function realpathSync(path: string): string {
  return sendSync("op_realpath", { path });
}

export function realpath(path: string): Promise<string> {
  return sendAsync("op_realpath", { path });
}
