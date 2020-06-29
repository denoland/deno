// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export function fdatasyncSync(rid: number): void {
  sendSync("op_fdatasync", { rid });
}

export async function fdatasync(rid: number): Promise<void> {
  await sendAsync("op_fdatasync", { rid });
}

export function fsyncSync(rid: number): void {
  sendSync("op_fsync", { rid });
}

export async function fsync(rid: number): Promise<void> {
  await sendAsync("op_fsync", { rid });
}
