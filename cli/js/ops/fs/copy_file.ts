// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync, sendAsync } from "../dispatch_json.ts";
import { pathFromURL } from "../../util.ts";

export function copyFileSync(
  fromPath: string | URL,
  toPath: string | URL,
): void {
  sendSync("op_copy_file", {
    from: pathFromURL(fromPath),
    to: pathFromURL(toPath),
  });
}

export async function copyFile(
  fromPath: string | URL,
  toPath: string | URL,
): Promise<void> {
  await sendAsync("op_copy_file", {
    from: pathFromURL(fromPath),
    to: pathFromURL(toPath),
  });
}
