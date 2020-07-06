// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";
import { pathFromURL } from "../../util.ts";

export interface RemoveOptions {
  recursive?: boolean;
}

export function removeSync(
  path: string | URL,
  options: RemoveOptions = {}
): void {
  path = pathFromURL(path);
  sendSync("op_remove", { path, recursive: !!options.recursive });
}

export async function remove(
  path: string | URL,
  options: RemoveOptions = {}
): Promise<void> {
  path = pathFromURL(path);
  await sendAsync("op_remove", { path, recursive: !!options.recursive });
}
