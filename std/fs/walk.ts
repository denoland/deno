// Documentation and interface for walk were adapted from Go
// https://golang.org/pkg/path/filepath/#Walk
// Copyright 2009 The Go Authors. All rights reserved. BSD license.
import { unimplemented } from "../testing/asserts.ts";
import { join } from "./path/mod.ts";
const { readDir, readDirSync, stat, statSync } = Deno;
type FileInfo = Deno.FileInfo;

export interface WalkOptions {
  maxDepth?: number;
  includeFiles?: boolean;
  includeDirs?: boolean;
  followSymlinks?: boolean;
  exts?: string[];
  match?: RegExp[];
  skip?: RegExp[];
  onError?: (err: Error) => void;
}

function patternTest(patterns: RegExp[], path: string): boolean {
  // Forced to reset last index on regex while iterating for have
  // consistent results.
  // See: https://stackoverflow.com/a/1520853
  return patterns.some((pattern): boolean => {
    const r = pattern.test(path);
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
 * - maxDepth?: number = Infinity;
 * - includeFiles?: boolean = true;
 * - includeDirs?: boolean = true;
 * - followSymlinks?: boolean = false;
 * - exts?: string[];
 * - match?: RegExp[];
 * - skip?: RegExp[];
 * - onError?: (err: Error) => void;
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
  const maxDepth = options.maxDepth != undefined ? options.maxDepth! : Infinity;
  if (maxDepth < 0) {
    return;
  }
  if (options.includeDirs != false && include(root, options)) {
    let rootInfo: FileInfo;
    try {
      rootInfo = await stat(root);
    } catch (err) {
      if (options.onError) {
        options.onError(err);
        return;
      }
    }
    yield { filename: root, info: rootInfo! };
  }
  if (maxDepth < 1 || patternTest(options.skip || [], root)) {
    return;
  }
  let ls: FileInfo[] = [];
  try {
    ls = await readDir(root);
  } catch (err) {
    if (options.onError) {
      options.onError(err);
    }
  }
  for (const info of ls) {
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
      if (options.includeFiles != false && include(filename, options)) {
        yield { filename, info };
      }
    } else {
      yield* walk(filename, { ...options, maxDepth: maxDepth - 1 });
    }
  }
}

/** Same as walk() but uses synchronous ops */
export function* walkSync(
  root: string,
  options: WalkOptions = {}
): IterableIterator<WalkInfo> {
  const maxDepth = options.maxDepth != undefined ? options.maxDepth! : Infinity;
  if (maxDepth < 0) {
    return;
  }
  if (options.includeDirs != false && include(root, options)) {
    let rootInfo: FileInfo;
    try {
      rootInfo = statSync(root);
    } catch (err) {
      if (options.onError) {
        options.onError(err);
        return;
      }
    }
    yield { filename: root, info: rootInfo! };
  }
  if (maxDepth < 1 || patternTest(options.skip || [], root)) {
    return;
  }
  let ls: FileInfo[] = [];
  try {
    ls = readDirSync(root);
  } catch (err) {
    if (options.onError) {
      options.onError(err);
    }
  }
  for (const info of ls) {
    if (info.isSymlink()) {
      if (options.followSymlinks) {
        unimplemented();
      } else {
        continue;
      }
    }

    const filename = join(root, info.name!);

    if (info.isFile()) {
      if (options.includeFiles != false && include(filename, options)) {
        yield { filename, info };
      }
    } else {
      yield* walkSync(filename, { ...options, maxDepth: maxDepth - 1 });
    }
  }
}
