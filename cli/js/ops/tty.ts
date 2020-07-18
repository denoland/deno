// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync } from "./dispatch_json.ts";

export function consoleSize(rid: number): [number, number] {
  return sendSync("op_console_size", { rid });
}

export function isatty(rid: number): boolean {
  return sendSync("op_isatty", { rid });
}

export function setRaw(rid: number, mode: boolean): void {
  sendSync("op_set_raw", { rid, mode });
}
