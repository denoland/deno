// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { stat, statSync } from "./ops/fs/stat.ts";
import { open, openSync } from "./files.ts";
import { chmod, chmodSync } from "./ops/fs/chmod.ts";
import { writeAll, writeAllSync } from "./buffer.ts";
import { build } from "./build.ts";

export interface WriteFileOptions {
  append?: boolean;
  create?: boolean;
  mode?: number;
}

export function writeFileSync(
  path: string,
  data: Uint8Array,
  options: WriteFileOptions = {}
): void {
  if (options.create !== undefined) {
    const create = !!options.create;
    if (!create) {
      // verify that file exists
      statSync(path);
    }
  }

  const openMode = !!options.append ? "a" : "w";
  const file = openSync(path, openMode);

  if (
    options.mode !== undefined &&
    options.mode !== null &&
    build.os !== "win"
  ) {
    chmodSync(path, options.mode);
  }

  writeAllSync(file, data);
  file.close();
}

export async function writeFile(
  path: string,
  data: Uint8Array,
  options: WriteFileOptions = {}
): Promise<void> {
  if (options.create !== undefined) {
    const create = !!options.create;
    if (!create) {
      // verify that file exists
      await stat(path);
    }
  }

  const openMode = !!options.append ? "a" : "w";
  const file = await open(path, openMode);

  if (
    options.mode !== undefined &&
    options.mode !== null &&
    build.os !== "win"
  ) {
    await chmod(path, options.mode);
  }

  await writeAll(file, data);
  file.close();
}
