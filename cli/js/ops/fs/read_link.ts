// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export function readlinkSync(path: string): string {
  return sendSync("op_read_link", { path });
}

export function readlink(path: string): Promise<string> {
  return sendAsync("op_read_link", { path });
}
