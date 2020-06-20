// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../core.ts";

export function bindSignal(signo: number): { rid: number } {
  return core.dispatchJson.sendSync("op_signal_bind", { signo });
}

export function pollSignal(rid: number): Promise<{ done: boolean }> {
  return core.dispatchJson.sendAsync("op_signal_poll", { rid });
}

export function unbindSignal(rid: number): void {
  core.dispatchJson.sendSync("op_signal_unbind", { rid });
}
