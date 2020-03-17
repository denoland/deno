// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync, sendAsync } from "./dispatch_json.ts";

export function startRepl(historyFile: string): number {
  return sendSync("op_repl_start", { historyFile });
}

export function readline(rid: number, prompt: string): Promise<string> {
  return sendAsync("op_repl_readline", { rid, prompt });
}
