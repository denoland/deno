const { readDir, readDirSync, readlink, readlinkSync, stat, statSync } = Deno;
import { FileInfo } from "deno";

export interface WalkOptions {
  maxDepth?: number;
  exts?: string[];
  match?: RegExp[];
  skip?: RegExp[];
  // FIXME don't use `any` here?
  onError?: (err: any) => void;
  followSymlinks?: Boolean;
}

/** Generate all files in a directory recursively.
 *
 *      for await (const fileInfo of walk()) {
 *        console.log(fileInfo.path);
 *        assert(fileInfo.isFile());
 *      };
 */
export async function* walk(
  dir: string = ".",
  options: WalkOptions = {}
): AsyncIterableIterator<FileInfo> {
  options.maxDepth -= 1;
  let ls: FileInfo[] = [];
  try {
    ls = await readDir(dir);
  } catch (err) {
    if (options.onError) {
      options.onError(err);
    }
  }
  for (let f of ls) {
    if (f.isSymlink()) {
      if (options.followSymlinks) {
        f = await resolve(f);
      } else {
        continue;
      }
    }
    if (f.isFile()) {
      if (include(f, options)) {
        yield f;
      }
    } else {
      if (!(options.maxDepth < 0)) {
        yield* walk(f.path, options);
      }
    }
  }
}

/** Generate all files in a directory recursively.
 *
 *      for (const fileInfo of walkSync()) {
 *        console.log(fileInfo.path);
 *        assert(fileInfo.isFile());
 *      };
 */
export function* walkSync(
  dir: string = ".",
  options: WalkOptions = {}
): IterableIterator<FileInfo> {
  options.maxDepth -= 1;
  let ls: FileInfo[] = [];
  try {
    ls = readDirSync(dir);
  } catch (err) {
    if (options.onError) {
      options.onError(err);
    }
  }
  for (let f of ls) {
    if (f.isSymlink()) {
      if (options.followSymlinks) {
        f = resolveSync(f);
      } else {
        continue;
      }
    }
    if (f.isFile()) {
      if (include(f, options)) {
        yield f;
      }
    } else {
      if (!(options.maxDepth < 0)) {
        yield* walkSync(f.path, options);
      }
    }
  }
}

function include(f: FileInfo, options: WalkOptions): Boolean {
  if (options.exts && !options.exts.some(ext => f.path.endsWith(ext))) {
    return false;
  }
  if (options.match && !options.match.some(pattern => pattern.test(f.path))) {
    return false;
  }
  if (options.skip && options.skip.some(pattern => pattern.test(f.path))) {
    return false;
  }
  return true;
}

async function resolve(f: FileInfo): Promise<FileInfo> {
  // This is the full path, unfortunately if we were to make it relative
  // it could resolve to a symlink and cause an infinite loop.
  const fpath = await readlink(f.path);
  f = await stat(fpath);
  // workaround path not being returned by stat
  f.path = fpath;
  return f;
}

function resolveSync(f: FileInfo): FileInfo {
  // This is the full path, unfortunately if we were to make it relative
  // it could resolve to a symlink and cause an infinite loop.
  const fpath = readlinkSync(f.path);
  f = statSync(fpath);
  // workaround path not being returned by stat
  f.path = fpath;
  return f;
}
