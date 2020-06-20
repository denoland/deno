// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../../core.ts";

export function cwd(): string {
  return core.dispatchJson.sendSync("op_cwd");
}

export function chdir(directory: string): void {
  core.dispatchJson.sendSync("op_chdir", { directory });
}
