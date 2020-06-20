// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../../core.ts";

export function linkSync(oldpath: string, newpath: string): void {
  core.dispatchJson.sendSync("op_link", { oldpath, newpath });
}

export async function link(oldpath: string, newpath: string): Promise<void> {
  await core.dispatchJson.sendAsync("op_link", { oldpath, newpath });
}
