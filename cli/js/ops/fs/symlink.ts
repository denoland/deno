// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export type symlinkOptions = {
  type: "file" | "dir";
};

export function symlinkSync(
  oldpath: string,
  newpath: string,
  options?: symlinkOptions
): void {
  sendSync("op_symlink", { oldpath, newpath, options });
}

export async function symlink(
  oldpath: string,
  newpath: string,
  options?: symlinkOptions
): Promise<void> {
  await sendAsync("op_symlink", { oldpath, newpath, options });
}
