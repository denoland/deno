// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync, sendAsync } from "./dispatch_json.ts";

interface BindSignalResponse {
  rid: number;
}

interface PollSignalResponse {
  done: boolean;
}

export function bindSignal(signo: number): BindSignalResponse {
  return sendSync("op_signal_bind", { signo });
}

export function pollSignal(rid: number): Promise<PollSignalResponse> {
  return sendAsync("op_signal_poll", { rid });
}

export function unbindSignal(rid: number): void {
  sendSync("op_signal_unbind", { rid });
}
