// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export function symlinkSync(
  oldpath: string,
  newpath: string,
  flag?: string
): void {
  sendSync("op_symlink", { oldpath, newpath, flag });
}

export async function symlink(
  oldpath: string,
  newpath: string,
  flag?: string
): Promise<void> {
  await sendAsync("op_symlink", { oldpath, newpath, flag });
}
