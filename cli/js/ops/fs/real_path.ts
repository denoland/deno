// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../../core.ts";

export function realPathSync(path: string): string {
  return core.dispatchJson.sendSync("op_realpath", { path });
}

export function realPath(path: string): Promise<string> {
  return core.dispatchJson.sendAsync("op_realpath", { path });
}
