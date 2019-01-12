/**
 * # deno-mkdirp
 *
 * `mkdir -p` 4 `deno`.
 *
 * ## Import
 *
 * ```ts
 * import { mkdirp } from "https://deno.land/x/std/fs/mkdirp.ts";
 * ```
 *
 * ## API
 *
 * Same as [`deno.mkdir`](https://deno.land/typedoc/index.html#mkdir).
 *
 * ### `mkdirp(path: string, mode?: number) : Promise<void>`
 *
 * Creates directories if they do not already exist and makes parent directories as needed.
 */
import { ErrorKind, FileInfo, lstat, mkdir, platform } from "deno";

const PATH_SEPARATOR: string = platform.os === "win" ? "\\" : "/";

export async function mkdirp(path: string, mode?: number): Promise<void> {
  for (
    let parts: string[] = path.split(/\/|\\/),
      parts_len: number = parts.length,
      level: string,
      info: FileInfo,
      i: number = 0;
    i < parts_len;
    i++
  ) {
    level = parts.slice(0, i + 1).join(PATH_SEPARATOR);
    try {
      info = await lstat(level);
      if (!info.isDirectory()) throw Error(`${level} is not a directory`);
    } catch (err) {
      if (err.kind !== ErrorKind.NotFound) throw err;
      await mkdir(level, mode);
    }
  }
}
