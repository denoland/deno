// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Documentation and interface for walk were adapted from Go
// https://golang.org/pkg/path/filepath/#Walk
// Copyright 2009 The Go Authors. All rights reserved. BSD license.
import { join } from "../path/join.ts";
import { normalize } from "../path/normalize.ts";
import {
  createWalkEntry,
  createWalkEntrySync,
  toPathString,
  WalkEntry,
} from "./_util.ts";

export class WalkError extends Error {
  override cause: unknown;
  override name = "WalkError";
  path: string;

  constructor(cause: unknown, path: string) {
    super(
      `${cause instanceof Error ? cause.message : cause} for path "${path}"`,
    );
    this.path = path;
    this.cause = cause;
  }
}

function include(
  path: string,
  exts?: string[],
  match?: RegExp[],
  skip?: RegExp[],
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

function wrapErrorWithPath(err: unknown, root: string) {
  if (err instanceof WalkError) return err;
  return new WalkError(err, root);
}

export interface WalkOptions {
  /**
   * The maximum depth of the file tree to be walked recursively.
   * @default {Infinity}
   */
  maxDepth?: number;
  /**
   * Indicates whether file entries should be included or not.
   * @default {true}
   */
  includeFiles?: boolean;
  /**
   * Indicates whether directory entries should be included or not.
   * @default {true}
   */
  includeDirs?: boolean;
  /**
   * Indicates whether symlink entries should be included or not.
   * This option is meaningful only if `followSymlinks` is set to `false`.
   * @default {true}
   */
  includeSymlinks?: boolean;
  /**
   * Indicates whether symlinks should be resolved or not.
   * @default {false}
   */
  followSymlinks?: boolean;
  /**
   * Indicates whether the followed symlink's path should be canonicalized.
   * This option works only if `followSymlinks` is not `false`.
   * @default {true}
   */
  canonicalize?: boolean;
  /**
   * List of file extensions used to filter entries.
   * If specified, entries without the file extension specified by this option are excluded.
   * @default {undefined}
   */
  exts?: string[];
  /**
   * List of regular expression patterns used to filter entries.
   * If specified, entries that do not match the patterns specified by this option are excluded.
   * @default {undefined}
   */
  match?: RegExp[];
  /**
   * List of regular expression patterns used to filter entries.
   * If specified, entries matching the patterns specified by this option are excluded.
   * @default {undefined}
   */
  skip?: RegExp[];
}
export type { WalkEntry };

/**
 * Walks the file tree rooted at root, yielding each file or directory in the
 * tree filtered according to the given options.
 *
 * @example
 * ```ts
 * import { walk } from "https://deno.land/std@$STD_VERSION/fs/walk.ts";
 * import { assert } from "https://deno.land/std@$STD_VERSION/assert/assert.ts";
 *
 * for await (const entry of walk(".")) {
 *   console.log(entry.path);
 *   assert(entry.isFile);
 * }
 * ```
 */
export async function* walk(
  root: string | URL,
  {
    maxDepth = Infinity,
    includeFiles = true,
    includeDirs = true,
    includeSymlinks = true,
    followSymlinks = false,
    canonicalize = true,
    exts = undefined,
    match = undefined,
    skip = undefined,
  }: WalkOptions = {},
): AsyncIterableIterator<WalkEntry> {
  if (maxDepth < 0) {
    return;
  }
  root = toPathString(root);
  if (includeDirs && include(root, exts, match, skip)) {
    yield await createWalkEntry(root);
  }
  if (maxDepth < 1 || !include(root, undefined, undefined, skip)) {
    return;
  }
  try {
    for await (const entry of Deno.readDir(root)) {
      let path = join(root, entry.name);

      let { isSymlink, isDirectory } = entry;

      if (isSymlink) {
        if (!followSymlinks) {
          if (includeSymlinks && include(path, exts, match, skip)) {
            yield { path, ...entry };
          }
          continue;
        }
        const realPath = await Deno.realPath(path);
        if (canonicalize) {
          path = realPath;
        }
        // Caveat emptor: don't assume |path| is not a symlink. realpath()
        // resolves symlinks but another process can replace the file system
        // entity with a different type of entity before we call lstat().
        ({ isSymlink, isDirectory } = await Deno.lstat(realPath));
      }

      if (isSymlink || isDirectory) {
        yield* walk(path, {
          maxDepth: maxDepth - 1,
          includeFiles,
          includeDirs,
          includeSymlinks,
          followSymlinks,
          exts,
          match,
          skip,
        });
      } else if (includeFiles && include(path, exts, match, skip)) {
        yield { path, ...entry };
      }
    }
  } catch (err) {
    throw wrapErrorWithPath(err, normalize(root));
  }
}

/** Same as walk() but uses synchronous ops */
export function* walkSync(
  root: string | URL,
  {
    maxDepth = Infinity,
    includeFiles = true,
    includeDirs = true,
    includeSymlinks = true,
    followSymlinks = false,
    canonicalize = true,
    exts = undefined,
    match = undefined,
    skip = undefined,
  }: WalkOptions = {},
): IterableIterator<WalkEntry> {
  root = toPathString(root);
  if (maxDepth < 0) {
    return;
  }
  if (includeDirs && include(root, exts, match, skip)) {
    yield createWalkEntrySync(root);
  }
  if (maxDepth < 1 || !include(root, undefined, undefined, skip)) {
    return;
  }
  let entries;
  try {
    entries = Deno.readDirSync(root);
  } catch (err) {
    throw wrapErrorWithPath(err, normalize(root));
  }
  for (const entry of entries) {
    let path = join(root, entry.name);

    let { isSymlink, isDirectory } = entry;

    if (isSymlink) {
      if (!followSymlinks) {
        if (includeSymlinks && include(path, exts, match, skip)) {
          yield { path, ...entry };
        }
        continue;
      }
      const realPath = Deno.realPathSync(path);
      if (canonicalize) {
        path = realPath;
      }
      // Caveat emptor: don't assume |path| is not a symlink. realpath()
      // resolves symlinks but another process can replace the file system
      // entity with a different type of entity before we call lstat().
      ({ isSymlink, isDirectory } = Deno.lstatSync(realPath));
    }

    if (isSymlink || isDirectory) {
      yield* walkSync(path, {
        maxDepth: maxDepth - 1,
        includeFiles,
        includeDirs,
        includeSymlinks,
        followSymlinks,
        exts,
        match,
        skip,
      });
    } else if (includeFiles && include(path, exts, match, skip)) {
      yield { path, ...entry };
    }
  }
}
