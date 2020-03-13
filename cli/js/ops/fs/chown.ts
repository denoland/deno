// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export function chownSync(path: string, uid: number, gid: number): void {
  sendSync("op_chown", { path, uid, gid });
}

export async function chown(
  path: string,
  uid: number,
  gid: number
): Promise<void> {
  await sendAsync("op_chown", { path, uid, gid });
}
