// Documentation and interface for walk were adapted from Go
// https://golang.org/pkg/path/filepath/#Walk
// Copyright 2009 The Go Authors. All rights reserved. BSD license.
import { unimplemented, assert } from "../testing/asserts.ts";
import { join } from "../path/mod.ts";
const { readdir, readdirSync, stat, statSync } = Deno;
type FileInfo = Deno.FileInfo;

export interface WalkOptions {
  maxDepth?: number;
  includeFiles?: boolean;
  includeDirs?: boolean;
  followSymlinks?: boolean;
  exts?: string[];
  match?: RegExp[];
  skip?: RegExp[];
}

function include(
  filename: string,
  exts?: string[],
  match?: RegExp[],
  skip?: RegExp[]
): boolean {
  if (exts && !exts.some((ext): boolean => filename.endsWith(ext))) {
    return false;
  }
  if (match && !match.some((pattern): boolean => !!filename.match(pattern))) {
    return false;
  }
  if (skip && skip.some((pattern): boolean => !!filename.match(pattern))) {
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
 *
 *      for await (const { filename, info } of walk(".")) {
 *        console.log(filename);
 *        assert(info.isFile());
 *      };
 */
export async function* walk(
  root: string,
  {
    maxDepth = Infinity,
    includeFiles = true,
    includeDirs = true,
    followSymlinks = false,
    exts = undefined,
    match = undefined,
    skip = undefined,
  }: WalkOptions = {}
): AsyncIterableIterator<WalkInfo> {
  if (maxDepth < 0) {
    return;
  }
  if (includeDirs && include(root, exts, match, skip)) {
    yield { filename: root, info: await stat(root) };
  }
  if (maxDepth < 1 || !include(root, undefined, undefined, skip)) {
    return;
  }
  const ls: FileInfo[] = await readdir(root);
  for (const info of ls) {
    if (info.isSymlink()) {
      if (followSymlinks) {
        // TODO(ry) Re-enable followSymlinks.
        unimplemented();
      } else {
        continue;
      }
    }

    assert(info.name != null);
    const filename = join(root, info.name);

    if (info.isFile()) {
      if (includeFiles && include(filename, exts, match, skip)) {
        yield { filename, info };
      }
    } else {
      yield* walk(filename, {
        maxDepth: maxDepth - 1,
        includeFiles,
        includeDirs,
        followSymlinks,
        exts,
        match,
        skip,
      });
    }
  }
}

/** Same as walk() but uses synchronous ops */
export function* walkSync(
  root: string,
  {
    maxDepth = Infinity,
    includeFiles = true,
    includeDirs = true,
    followSymlinks = false,
    exts = undefined,
    match = undefined,
    skip = undefined,
  }: WalkOptions = {}
): IterableIterator<WalkInfo> {
  if (maxDepth < 0) {
    return;
  }
  if (includeDirs && include(root, exts, match, skip)) {
    yield { filename: root, info: statSync(root) };
  }
  if (maxDepth < 1 || !include(root, undefined, undefined, skip)) {
    return;
  }
  const ls: FileInfo[] = readdirSync(root);
  for (const info of ls) {
    if (info.isSymlink()) {
      if (followSymlinks) {
        unimplemented();
      } else {
        continue;
      }
    }

    assert(info.name != null);
    const filename = join(root, info.name);

    if (info.isFile()) {
      if (includeFiles && include(filename, exts, match, skip)) {
        yield { filename, info };
      }
    } else {
      yield* walkSync(filename, {
        maxDepth: maxDepth - 1,
        includeFiles,
        includeDirs,
        followSymlinks,
        exts,
        match,
        skip,
      });
    }
  }
}
