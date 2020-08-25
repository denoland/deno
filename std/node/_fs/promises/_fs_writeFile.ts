// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import type { Encodings, WriteFileOptions } from "../_fs_common.ts";

import { writeFile as writeFileCallback } from "../_fs_writeFile.ts";

export function writeFile(
  pathOrRid: string | number | URL,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
): Promise<void> {
  return new Promise((resolve, reject) => {
    writeFileCallback(pathOrRid, data, options, (err?: Error | null) => {
      if (err) return reject(err);
      resolve();
    });
  });
}
