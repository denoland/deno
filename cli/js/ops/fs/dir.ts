// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync } from "../dispatch_json.ts";

export function cwd(): string {
  return sendSync("op_cwd");
}

export function chdir(directory: string): void {
  sendSync("op_chdir", { directory });
}
