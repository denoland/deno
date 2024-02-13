// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { GlobOptions, globToRegExp, isGlob, joinGlobs } from "../path/glob.ts";
import { isAbsolute } from "../path/is_absolute.ts";
import { resolve } from "../path/resolve.ts";
import { SEP_PATTERN } from "../path/separator.ts";
import { walk, walkSync } from "./walk.ts";
import { assert } from "../assert/assert.ts";
import {
  createWalkEntry,
  createWalkEntrySync,
  toPathString,
  WalkEntry,
} from "./_util.ts";

const isWindows = Deno.build.os === "windows";

export interface ExpandGlobOptions extends Omit<GlobOptions, "os"> {
  root?: string;
  exclude?: string[];
  includeDirs?: boolean;
  followSymlinks?: boolean;
  /**
   * Indicates whether the followed symlink's path should be canonicalized.
   * This option works only if `followSymlinks` is not `false`.
   * @default {true}
   */
  canonicalize?: boolean;
}

interface SplitPath {
  segments: string[];
  isAbsolute: boolean;
  hasTrailingSep: boolean;
  // Defined for any absolute Windows path.
  winRoot?: string;
}

function split(path: string): SplitPath {
  const s = SEP_PATTERN.source;
  const segments = path
    .replace(new RegExp(`^${s}|${s}$`, "g"), "")
    .split(SEP_PATTERN);
  const isAbsolute_ = isAbsolute(path);
  return {
    segments,
    isAbsolute: isAbsolute_,
    hasTrailingSep: !!path.match(new RegExp(`${s}$`)),
    winRoot: isWindows && isAbsolute_ ? segments.shift() : undefined,
  };
}

function throwUnlessNotFound(error: unknown) {
  if (!(error instanceof Deno.errors.NotFound)) {
    throw error;
  }
}

function comparePath(a: WalkEntry, b: WalkEntry): number {
  if (a.path < b.path) return -1;
  if (a.path > b.path) return 1;
  return 0;
}

/**
 * Expand the glob string from the specified `root` directory and yield each
 * result as a `WalkEntry` object.
 *
 * See [`globToRegExp()`](../path/glob.ts#globToRegExp) for details on supported
 * syntax.
 *
 * @example
 * ```ts
 * import { expandGlob } from "https://deno.land/std@$STD_VERSION/fs/expand_glob.ts";
 * for await (const file of expandGlob("**\/*.ts")) {
 *   console.log(file);
 * }
 * ```
 */
export async function* expandGlob(
  glob: string | URL,
  {
    root,
    exclude = [],
    includeDirs = true,
    extended = true,
    globstar = true,
    caseInsensitive,
    followSymlinks,
    canonicalize,
  }: ExpandGlobOptions = {},
): AsyncIterableIterator<WalkEntry> {
  const {
    segments,
    isAbsolute: isGlobAbsolute,
    hasTrailingSep,
    winRoot,
  } = split(toPathString(glob));
  root ??= isGlobAbsolute ? winRoot ?? "/" : Deno.cwd();

  const globOptions: GlobOptions = { extended, globstar, caseInsensitive };
  const absRoot = isGlobAbsolute ? root : resolve(root!); // root is always string here
  const resolveFromRoot = (path: string): string => resolve(absRoot, path);
  const excludePatterns = exclude
    .map(resolveFromRoot)
    .map((s: string): RegExp => globToRegExp(s, globOptions));
  const shouldInclude = (path: string): boolean =>
    !excludePatterns.some((p: RegExp): boolean => !!path.match(p));

  let fixedRoot = isGlobAbsolute
    ? winRoot !== undefined ? winRoot : "/"
    : absRoot;
  while (segments.length > 0 && !isGlob(segments[0])) {
    const seg = segments.shift();
    assert(seg !== undefined);
    fixedRoot = joinGlobs([fixedRoot, seg], globOptions);
  }

  let fixedRootInfo: WalkEntry;
  try {
    fixedRootInfo = await createWalkEntry(fixedRoot);
  } catch (error) {
    return throwUnlessNotFound(error);
  }

  async function* advanceMatch(
    walkInfo: WalkEntry,
    globSegment: string,
  ): AsyncIterableIterator<WalkEntry> {
    if (!walkInfo.isDirectory) {
      return;
    } else if (globSegment === "..") {
      const parentPath = joinGlobs([walkInfo.path, ".."], globOptions);
      try {
        if (shouldInclude(parentPath)) {
          return yield await createWalkEntry(parentPath);
        }
      } catch (error) {
        throwUnlessNotFound(error);
      }
      return;
    } else if (globSegment === "**") {
      return yield* walk(walkInfo.path, {
        skip: excludePatterns,
        maxDepth: globstar ? Infinity : 1,
        followSymlinks,
        canonicalize,
      });
    }
    const globPattern = globToRegExp(globSegment, globOptions);
    for await (
      const walkEntry of walk(walkInfo.path, {
        maxDepth: 1,
        skip: excludePatterns,
        followSymlinks,
      })
    ) {
      if (
        walkEntry.path !== walkInfo.path &&
        walkEntry.name.match(globPattern)
      ) {
        yield walkEntry;
      }
    }
  }

  let currentMatches: WalkEntry[] = [fixedRootInfo];
  for (const segment of segments) {
    // Advancing the list of current matches may introduce duplicates, so we
    // pass everything through this Map.
    const nextMatchMap: Map<string, WalkEntry> = new Map();
    await Promise.all(
      currentMatches.map(async (currentMatch) => {
        for await (const nextMatch of advanceMatch(currentMatch, segment)) {
          nextMatchMap.set(nextMatch.path, nextMatch);
        }
      }),
    );
    currentMatches = [...nextMatchMap.values()].sort(comparePath);
  }

  if (hasTrailingSep) {
    currentMatches = currentMatches.filter(
      (entry: WalkEntry): boolean => entry.isDirectory,
    );
  }
  if (!includeDirs) {
    currentMatches = currentMatches.filter(
      (entry: WalkEntry): boolean => !entry.isDirectory,
    );
  }
  yield* currentMatches;
}

/**
 * Synchronous version of `expandGlob()`.
 *
 * @example
 * ```ts
 * import { expandGlobSync } from "https://deno.land/std@$STD_VERSION/fs/expand_glob.ts";
 * for (const file of expandGlobSync("**\/*.ts")) {
 *   console.log(file);
 * }
 * ```
 */
export function* expandGlobSync(
  glob: string | URL,
  {
    root,
    exclude = [],
    includeDirs = true,
    extended = true,
    globstar = true,
    caseInsensitive,
    followSymlinks,
    canonicalize,
  }: ExpandGlobOptions = {},
): IterableIterator<WalkEntry> {
  const {
    segments,
    isAbsolute: isGlobAbsolute,
    hasTrailingSep,
    winRoot,
  } = split(toPathString(glob));
  root ??= isGlobAbsolute ? winRoot ?? "/" : Deno.cwd();

  const globOptions: GlobOptions = { extended, globstar, caseInsensitive };
  const absRoot = isGlobAbsolute ? root : resolve(root!); // root is always string here
  const resolveFromRoot = (path: string): string => resolve(absRoot, path);
  const excludePatterns = exclude
    .map(resolveFromRoot)
    .map((s: string): RegExp => globToRegExp(s, globOptions));
  const shouldInclude = (path: string): boolean =>
    !excludePatterns.some((p: RegExp): boolean => !!path.match(p));

  let fixedRoot = isGlobAbsolute
    ? winRoot !== undefined ? winRoot : "/"
    : absRoot;
  while (segments.length > 0 && !isGlob(segments[0])) {
    const seg = segments.shift();
    assert(seg !== undefined);
    fixedRoot = joinGlobs([fixedRoot, seg], globOptions);
  }

  let fixedRootInfo: WalkEntry;
  try {
    fixedRootInfo = createWalkEntrySync(fixedRoot);
  } catch (error) {
    return throwUnlessNotFound(error);
  }

  function* advanceMatch(
    walkInfo: WalkEntry,
    globSegment: string,
  ): IterableIterator<WalkEntry> {
    if (!walkInfo.isDirectory) {
      return;
    } else if (globSegment === "..") {
      const parentPath = joinGlobs([walkInfo.path, ".."], globOptions);
      try {
        if (shouldInclude(parentPath)) {
          return yield createWalkEntrySync(parentPath);
        }
      } catch (error) {
        throwUnlessNotFound(error);
      }
      return;
    } else if (globSegment === "**") {
      return yield* walkSync(walkInfo.path, {
        skip: excludePatterns,
        maxDepth: globstar ? Infinity : 1,
        followSymlinks,
        canonicalize,
      });
    }
    const globPattern = globToRegExp(globSegment, globOptions);
    for (
      const walkEntry of walkSync(walkInfo.path, {
        maxDepth: 1,
        skip: excludePatterns,
        followSymlinks,
      })
    ) {
      if (
        walkEntry.path !== walkInfo.path &&
        walkEntry.name.match(globPattern)
      ) {
        yield walkEntry;
      }
    }
  }

  let currentMatches: WalkEntry[] = [fixedRootInfo];
  for (const segment of segments) {
    // Advancing the list of current matches may introduce duplicates, so we
    // pass everything through this Map.
    const nextMatchMap: Map<string, WalkEntry> = new Map();
    for (const currentMatch of currentMatches) {
      for (const nextMatch of advanceMatch(currentMatch, segment)) {
        nextMatchMap.set(nextMatch.path, nextMatch);
      }
    }
    currentMatches = [...nextMatchMap.values()].sort(comparePath);
  }

  if (hasTrailingSep) {
    currentMatches = currentMatches.filter(
      (entry: WalkEntry): boolean => entry.isDirectory,
    );
  }
  if (!includeDirs) {
    currentMatches = currentMatches.filter(
      (entry: WalkEntry): boolean => !entry.isDirectory,
    );
  }
  yield* currentMatches;
}
