// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { FileOptions } from "../_fs_common.ts";
import { MaybeEmpty } from "../../_utils.ts";

import { readFile as readFileCallback } from "../_fs_readFile.ts";

export function readFile(
  path: string | URL,
  options?: FileOptions | string
): Promise<MaybeEmpty<string | Uint8Array>> {
  return new Promise((resolve, reject) => {
    readFileCallback(path, options, (err, data): void => {
      if (err) return reject(err);
      resolve(data);
    });
  });
}
