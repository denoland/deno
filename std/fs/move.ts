// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { exists, existsSync } from "./exists.ts";
import { isSubdir } from "./_util.ts";

interface MoveOptions {
  overwrite?: boolean;
}

/** Moves a file or directory */
export async function move(
  src: string | URL,
  dest: string | URL,
  { overwrite = false }: MoveOptions = {},
): Promise<void> {
  const srcStat = await Deno.stat(src);

  if (srcStat.isDirectory && isSubdir(src, dest)) {
    throw new Error(
      `Cannot move '${src}' to a subdirectory of itself, '${dest}'.`,
    );
  }

  if (overwrite) {
    if (await exists(dest)) {
      await Deno.remove(dest, { recursive: true });
    }
  } else {
    if (await exists(dest)) {
      throw new Error("dest already exists.");
    }
  }

  await Deno.rename(src, dest);

  return;
}

/** Moves a file or directory synchronously */
export function moveSync(
  src: string | URL,
  dest: string | URL,
  { overwrite = false }: MoveOptions = {},
): void {
  const srcStat = Deno.statSync(src);

  if (srcStat.isDirectory && isSubdir(src, dest)) {
    throw new Error(
      `Cannot move '${src}' to a subdirectory of itself, '${dest}'.`,
    );
  }

  if (overwrite) {
    if (existsSync(dest)) {
      Deno.removeSync(dest, { recursive: true });
    }
  } else {
    if (existsSync(dest)) {
      throw new Error("dest already exists.");
    }
  }

  Deno.renameSync(src, dest);
}
