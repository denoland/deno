// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export function chmodSync(path: string, mode: number): void {
  sendSync("op_chmod", { path, mode });
}

export async function chmod(path: string, mode: number): Promise<void> {
  await sendAsync("op_chmod", { path, mode });
}
