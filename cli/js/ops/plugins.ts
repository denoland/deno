// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync } from "./dispatch_json.ts";

export function openPlugin(filename: string): number {
  return sendSync("op_open_plugin", { filename });
}
