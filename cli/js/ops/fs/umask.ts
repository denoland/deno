// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../../core.ts";

export function umask(mask?: number): number {
  return core.dispatchJson.sendSync("op_umask", { mask });
}
