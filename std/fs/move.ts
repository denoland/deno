// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { exists, existsSync } from "./exists.ts";
import { isSubdir } from "./utils.ts";

interface MoveOptions {
  overwrite?: boolean;
}

/** Moves a file or directory */
export async function move(
  src: string,
  dest: string,
  { overwrite = false }: MoveOptions = {}
): Promise<void> {
  const srcStat = await Deno.stat(src);

  if (srcStat.isDirectory() && isSubdir(src, dest)) {
    throw new Error(
      `Cannot move '${src}' to a subdirectory of itself, '${dest}'.`
    );
  }

  if (overwrite) {
    await Deno.remove(dest, { recursive: true });
    await Deno.rename(src, dest);
  } else {
    if (await exists(dest)) {
      throw new Error("dest already exists.");
    }
    await Deno.rename(src, dest);
  }

  return;
}

/** Moves a file or directory */
export function moveSync(
  src: string,
  dest: string,
  { overwrite = false }: MoveOptions = {}
): void {
  const srcStat = Deno.statSync(src);

  if (srcStat.isDirectory() && isSubdir(src, dest)) {
    throw new Error(
      `Cannot move '${src}' to a subdirectory of itself, '${dest}'.`
    );
  }

  if (overwrite) {
    Deno.removeSync(dest, { recursive: true });
    Deno.renameSync(src, dest);
  } else {
    if (existsSync(dest)) {
      throw new Error("dest already exists.");
    }
    Deno.renameSync(src, dest);
  }
}
