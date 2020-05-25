// Documentation and interface for walk were adapted from Go
// https://golang.org/pkg/path/filepath/#Walk
// Copyright 2009 The Go Authors. All rights reserved. BSD license.
import { assert } from "../_util/assert.ts";
import { basename, join, normalize } from "../path/mod.ts";
const { readDir, readDirSync, stat, statSync } = Deno;

export function createWalkEntrySync(path: string): WalkEntry {
  path = normalize(path);
  const name = basename(path);
  const info = statSync(path);
  return {
    path,
    name,
    isFile: info.isFile,
    isDirectory: info.isDirectory,
    isSymlink: info.isSymlink,
  };
}

export async function createWalkEntry(path: string): Promise<WalkEntry> {
  path = normalize(path);
  const name = basename(path);
  const info = await stat(path);
  return {
    path,
    name,
    isFile: info.isFile,
    isDirectory: info.isDirectory,
    isSymlink: info.isSymlink,
  };
}

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
  path: string,
  exts?: string[],
  match?: RegExp[],
  skip?: RegExp[]
): boolean {
  if (exts && !exts.some((ext): boolean => path.endsWith(ext))) {
    return false;
  }
  if (match && !match.some((pattern): boolean => !!path.match(pattern))) {
    return false;
  }
  if (skip && skip.some((pattern): boolean => !!path.match(pattern))) {
    return false;
  }
  return true;
}

export interface WalkEntry extends Deno.DirEntry {
  path: string;
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
 *      for await (const entry of walk(".")) {
 *        console.log(entry.path);
 *        assert(entry.isFile);
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
): AsyncIterableIterator<WalkEntry> {
  if (maxDepth < 0) {
    return;
  }
  if (includeDirs && include(root, exts, match, skip)) {
    yield await createWalkEntry(root);
  }
  if (maxDepth < 1 || !include(root, undefined, undefined, skip)) {
    return;
  }
  for await (const entry of readDir(root)) {
    if (entry.isSymlink) {
      if (followSymlinks) {
        // TODO(ry) Re-enable followSymlinks.
        throw new Error("unimplemented");
      } else {
        continue;
      }
    }

    assert(entry.name != null);
    const path = join(root, entry.name);

    if (entry.isFile) {
      if (includeFiles && include(path, exts, match, skip)) {
        yield { path, ...entry };
      }
    } else {
      yield* walk(path, {
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
): IterableIterator<WalkEntry> {
  if (maxDepth < 0) {
    return;
  }
  if (includeDirs && include(root, exts, match, skip)) {
    yield createWalkEntrySync(root);
  }
  if (maxDepth < 1 || !include(root, undefined, undefined, skip)) {
    return;
  }
  for (const entry of readDirSync(root)) {
    if (entry.isSymlink) {
      if (followSymlinks) {
        throw new Error("unimplemented");
      } else {
        continue;
      }
    }

    assert(entry.name != null);
    const path = join(root, entry.name);

    if (entry.isFile) {
      if (includeFiles && include(path, exts, match, skip)) {
        yield { path, ...entry };
      }
    } else {
      yield* walkSync(path, {
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
