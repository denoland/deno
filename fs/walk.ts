const { readDir, readDirSync, readlink, readlinkSync, stat, statSync } = Deno;
type FileInfo = Deno.FileInfo;

export interface WalkOptions {
  maxDepth?: number;
  exts?: string[];
  match?: RegExp[];
  skip?: RegExp[];
  onError?: (err: Error) => void;
  followSymlinks?: boolean;
}

function patternTest(patterns: RegExp[], path: string): boolean {
  // Forced to reset last index on regex while iterating for have
  // consistent results.
  // See: https://stackoverflow.com/a/1520853
  return patterns.some(
    (pattern): boolean => {
      let r = pattern.test(path);
      pattern.lastIndex = 0;
      return r;
    }
  );
}

function include(f: FileInfo, options: WalkOptions): boolean {
  if (
    options.exts &&
    !options.exts.some((ext): boolean => f.path.endsWith(ext))
  ) {
    return false;
  }
  if (options.match && !patternTest(options.match, f.path)) {
    return false;
  }
  if (options.skip && patternTest(options.skip, f.path)) {
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
  const length = ls.length;
  for (var i = 0; i < length; i++) {
    let f = ls[i];
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
  const length = ls.length;
  for (var i = 0; i < length; i++) {
    let f = ls[i];
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
