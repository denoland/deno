// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../../core.ts";
import { pathFromURL } from "../../util.ts";

export interface RemoveOptions {
  recursive?: boolean;
}

export function removeSync(
  path: string | URL,
  options: RemoveOptions = {}
): void {
  path = pathFromURL(path);
  core.dispatchJson.sendSync("op_remove", {
    path,
    recursive: !!options.recursive,
  });
}

export async function remove(
  path: string | URL,
  options: RemoveOptions = {}
): Promise<void> {
  path = pathFromURL(path);
  await core.dispatchJson.sendAsync("op_remove", {
    path,
    recursive: !!options.recursive,
  });
}
