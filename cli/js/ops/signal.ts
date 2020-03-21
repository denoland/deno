// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";

export function bindSignal(signo: number): { rid: number } {
  return sendSync("op_signal_bind", { signo });
}

export function pollSignal(rid: number): Promise<{ done: boolean }> {
  return sendAsync("op_signal_poll", { rid });
}

export function unbindSignal(rid: number): void {
  sendSync("op_signal_unbind", { rid });
}
