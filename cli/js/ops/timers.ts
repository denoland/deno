// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";

export function stopGlobalTimer(): void {
  sendSync("op_global_timer_stop");
}

export async function startGlobalTimer(timeout: number): Promise<void> {
  await sendAsync("op_global_timer", { timeout });
}

interface NowResponse {
  seconds: number;
  subsecNanos: number;
}

export function now(): NowResponse {
  return sendSync("op_now");
}
