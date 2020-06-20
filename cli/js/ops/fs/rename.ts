// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../../core.ts";

export function renameSync(oldpath: string, newpath: string): void {
  core.dispatchJson.sendSync("op_rename", { oldpath, newpath });
}

export async function rename(oldpath: string, newpath: string): Promise<void> {
  await core.dispatchJson.sendAsync("op_rename", { oldpath, newpath });
}
