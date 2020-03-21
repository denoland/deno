// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export function renameSync(oldpath: string, newpath: string): void {
  sendSync("op_rename", { oldpath, newpath });
}

export async function rename(oldpath: string, newpath: string): Promise<void> {
  await sendAsync("op_rename", { oldpath, newpath });
}
