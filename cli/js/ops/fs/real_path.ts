// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export function realPathSync(path: string): string {
  return sendSync("op_realpath", { path });
}

export function realPath(path: string): Promise<string> {
  return sendAsync("op_realpath", { path });
}
