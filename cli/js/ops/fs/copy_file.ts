// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export function copyFileSync(fromPath: string, toPath: string): void {
  sendSync("op_copy_file", { from: fromPath, to: toPath });
}

export async function copyFile(
  fromPath: string,
  toPath: string
): Promise<void> {
  await sendAsync("op_copy_file", { from: fromPath, to: toPath });
}
