// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { core } from "../core.ts";

export function startRepl(historyFile: string): number {
  return core.dispatchJson.sendSync("op_repl_start", { historyFile });
}

export function readline(rid: number, prompt: string): Promise<string> {
  return core.dispatchJson.sendAsync("op_repl_readline", { rid, prompt });
}
