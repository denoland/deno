// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../../core.ts";

export function readLinkSync(path: string): string {
  return core.dispatchJson.sendSync("op_read_link", { path });
}

export function readLink(path: string): Promise<string> {
  return core.dispatchJson.sendAsync("op_read_link", { path });
}
