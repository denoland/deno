// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";
import { pathFromURL } from "../../util.ts";

export function copyFileSync(fromPath: string | URL, toPath: string | URL): void {
  if (fromPath instanceof URL) {
    fromPath = pathFromURL(fromPath);
  }
  if (toPath instanceof URL) {
    toPath = pathFromURL(toPath);
  }
  sendSync("op_copy_file", { from: fromPath, to: toPath });
}

export async function copyFile(
  fromPath: string | URL,
  toPath: string | URL
): Promise<void> {
  if (fromPath instanceof URL) {
    fromPath = pathFromURL(fromPath);
  }
  if (toPath instanceof URL) {
    toPath = pathFromURL(toPath);
  }
  await sendAsync("op_copy_file", { from: fromPath, to: toPath });
}
