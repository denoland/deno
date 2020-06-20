// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../core.ts";

export function stopGlobalTimer(): void {
  core.dispatchJson.sendSync("op_global_timer_stop");
}

export async function startGlobalTimer(timeout: number): Promise<void> {
  await core.dispatchJson.sendAsync("op_global_timer", { timeout });
}

interface NowResponse {
  seconds: number;
  subsecNanos: number;
}

export function now(): NowResponse {
  return core.dispatchJson.sendSync("op_now");
}
