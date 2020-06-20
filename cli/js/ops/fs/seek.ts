// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../../core.ts";
import { SeekMode } from "../../io.ts";

export function seekSync(
  rid: number,
  offset: number,
  whence: SeekMode
): number {
  return core.dispatchJson.sendSync("op_seek", { rid, offset, whence });
}

export function seek(
  rid: number,
  offset: number,
  whence: SeekMode
): Promise<number> {
  return core.dispatchJson.sendAsync("op_seek", { rid, offset, whence });
}
