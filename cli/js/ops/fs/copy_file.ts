// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../../core.ts";
import { pathFromURL } from "../../util.ts";

export function copyFileSync(
  fromPath: string | URL,
  toPath: string | URL
): void {
  fromPath = pathFromURL(fromPath);
  toPath = pathFromURL(toPath);

  core.dispatchJson.sendSync("op_copy_file", { from: fromPath, to: toPath });
}

export async function copyFile(
  fromPath: string | URL,
  toPath: string | URL
): Promise<void> {
  fromPath = pathFromURL(fromPath);
  toPath = pathFromURL(toPath);

  await core.dispatchJson.sendAsync("op_copy_file", {
    from: fromPath,
    to: toPath,
  });
}
