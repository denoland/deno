// Documentation and interface for walk were adapted from Go
// https://golang.org/pkg/path/filepath/#Walk
// Copyright 2009 The Go Authors. All rights reserved. BSD license.
const { readDir, readDirSync, stat, statSync } = Deno;
type FileInfo = Deno.FileInfo;
import { unimplemented } from "../testing/asserts.ts";
import { join } from "./path/mod.ts";

export interface WalkOptions {
  maxDepth?: number;
  includeDirs?: boolean;
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
  return patterns.some((pattern): boolean => {
    let r = pattern.test(path);
    pattern.lastIndex = 0;
    return r;
  });
}

function include(filename: string, options: WalkOptions): boolean {
  if (
    options.exts &&
    !options.exts.some((ext): boolean => filename.endsWith(ext))
  ) {
    return false;
  }
  if (options.match && !patternTest(options.match, filename)) {
    return false;
  }
  if (options.skip && patternTest(options.skip, filename)) {
    return false;
  }
  return true;
}

export interface WalkInfo {
  filename: string;
  info: FileInfo;
}

/** Walks the file tree rooted at root, yielding each file or directory in the
 * tree filtered according to the given options. The files are walked in lexical
 * order, which makes the output deterministic but means that for very large
 * directories walk() can be inefficient.
 *
 * Options:
 * - maxDepth?: number;
 * - includeDirs?: boolean;
 * - exts?: string[];
 * - match?: RegExp[];
 * - skip?: RegExp[];
 * - onError?: (err: Error) => void;
 * - followSymlinks?: boolean;
 *
 *      for await (const { filename, info } of walk(".")) {
 *        console.log(filename);
 *        assert(info.isFile());
 *      };
 */
export async function* walk(
  root: string,
  options: WalkOptions = {}
): AsyncIterableIterator<WalkInfo> {
  options.maxDepth! -= 1;
  if (options.includeDirs && include(root, options)) {
    const rootInfo = await stat(root);
    yield { filename: root, info: rootInfo };
  }
  let ls: FileInfo[] = [];
  try {
    ls = await readDir(root);
  } catch (err) {
    if (options.onError) {
      options.onError(err);
    }
  }
  for (let info of ls) {
    if (info.isSymlink()) {
      if (options.followSymlinks) {
        // TODO(ry) Re-enable followSymlinks.
        unimplemented();
      } else {
        continue;
      }
    }

    const filename = join(root, info.name!);

    if (info.isFile()) {
      if (include(filename, options)) {
        yield { filename, info };
      }
    } else {
      if (!(options.maxDepth! < 0)) {
        yield* walk(filename, options);
      }
    }
  }
}

/** Same as walk() but uses synchronous ops */
export function* walkSync(
  root: string = ".",
  options: WalkOptions = {}
): IterableIterator<WalkInfo> {
  options.maxDepth! -= 1;
  if (options.includeDirs && include(root, options)) {
    const rootInfo = statSync(root);
    yield { filename: root, info: rootInfo };
  }
  let ls: FileInfo[] = [];
  try {
    ls = readDirSync(root);
  } catch (err) {
    if (options.onError) {
      options.onError(err);
    }
  }
  for (let info of ls) {
    if (info.isSymlink()) {
      if (options.followSymlinks) {
        unimplemented();
      } else {
        continue;
      }
    }

    const filename = join(root, info.name!);

    if (info.isFile()) {
      if (include(filename, options)) {
        yield { filename, info };
      }
    } else {
      if (!(options.maxDepth! < 0)) {
        yield* walkSync(filename, options);
      }
    }
  }
}
