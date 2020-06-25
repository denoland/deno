// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export function fsyncSync(rid: number): void {
  sendSync("op_fsync", { rid });
}

export async function fsync(rid: number): Promise<void> {
  await sendAsync("op_fsync", { rid });
}
