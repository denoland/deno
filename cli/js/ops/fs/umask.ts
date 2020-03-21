// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync } from "../dispatch_json.ts";

export function umask(mask?: number): number {
  return sendSync("op_umask", { mask });
}
