// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../../core.ts";

export type symlinkOptions = {
  type: "file" | "dir";
};

export function symlinkSync(
  oldpath: string,
  newpath: string,
  options?: symlinkOptions
): void {
  core.dispatchJson.sendSync("op_symlink", { oldpath, newpath, options });
}

export async function symlink(
  oldpath: string,
  newpath: string,
  options?: symlinkOptions
): Promise<void> {
  await core.dispatchJson.sendAsync("op_symlink", {
    oldpath,
    newpath,
    options,
  });
}
