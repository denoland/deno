// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export interface RemoveOptions {
  recursive?: boolean;
}

export function removeSync(path: string, options: RemoveOptions = {}): void {
  sendSync("op_remove", { path, recursive: !!options.recursive });
}

export async function remove(
  path: string,
  options: RemoveOptions = {}
): Promise<void> {
  await sendAsync("op_remove", { path, recursive: !!options.recursive });
}
