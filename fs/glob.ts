import { globrex } from "./globrex.ts";
import { SEP, SEP_PATTERN, isWindows } from "./path/constants.ts";
import { isAbsolute, join, normalize } from "./path/mod.ts";
import { WalkInfo, walk, walkSync } from "./walk.ts";
const { DenoError, ErrorKind, cwd, stat, statSync } = Deno;
type FileInfo = Deno.FileInfo;

export interface GlobOptions {
  extended?: boolean;
  globstar?: boolean;
}

export interface GlobToRegExpOptions extends GlobOptions {
  flags?: string;
}

/**
 * Generate a regex based on glob pattern and options
 * This was meant to be using the the `fs.walk` function
 * but can be used anywhere else.
 * Examples:
 *
 *     Looking for all the `ts` files:
 *     walkSync(".", {
 *       match: [globToRegExp("*.ts")]
 *     })
 *
 *     Looking for all the `.json` files in any subfolder:
 *     walkSync(".", {
 *       match: [globToRegExp(join("a", "**", "*.json"),{
 *         flags: "g",
 *         extended: true,
 *         globstar: true
 *       })]
 *     })
 *
 * @param glob - Glob pattern to be used
 * @param options - Specific options for the glob pattern
 * @returns A RegExp for the glob pattern
 */
export function globToRegExp(
  glob: string,
  options: GlobToRegExpOptions = {}
): RegExp {
  const result = globrex(glob, { ...options, strict: false, filepath: true });
  return result.path!.regex;
}

/** Test whether the given string is a glob */
export function isGlob(str: string): boolean {
  const chars: Record<string, string> = { "{": "}", "(": ")", "[": "]" };
  /* eslint-disable-next-line max-len */
  const regex = /\\(.)|(^!|\*|[\].+)]\?|\[[^\\\]]+\]|\{[^\\}]+\}|\(\?[:!=][^\\)]+\)|\([^|]+\|[^\\)]+\))/;

  if (str === "") {
    return false;
  }

  let match: RegExpExecArray | null;

  while ((match = regex.exec(str))) {
    if (match[2]) return true;
    let idx = match.index + match[0].length;

    // if an open bracket/brace/paren is escaped,
    // set the index to the next closing character
    const open = match[1];
    const close = open ? chars[open] : null;
    if (open && close) {
      const n = str.indexOf(close, idx);
      if (n !== -1) {
        idx = n + 1;
      }
    }

    str = str.slice(idx);
  }

  return false;
}

/** Like normalize(), but doesn't collapse "**\/.." when `globstar` is true. */
export function normalizeGlob(
  glob: string,
  { globstar = false }: GlobOptions = {}
): string {
  if (!!glob.match(/\0/g)) {
    throw new DenoError(
      ErrorKind.InvalidPath,
      `Glob contains invalid characters: "${glob}"`
    );
  }
  if (!globstar) {
    return normalize(glob);
  }
  const s = SEP_PATTERN.source;
  const badParentPattern = new RegExp(
    `(?<=(${s}|^)\\*\\*${s})\\.\\.(?=${s}|$)`,
    "g"
  );
  return normalize(glob.replace(badParentPattern, "\0")).replace(/\0/g, "..");
}

/** Like join(), but doesn't collapse "**\/.." when `globstar` is true. */
export function joinGlobs(
  globs: string[],
  { extended = false, globstar = false }: GlobOptions = {}
): string {
  if (!globstar || globs.length == 0) {
    return join(...globs);
  }
  if (globs.length === 0) return ".";
  let joined: string | undefined;
  for (const glob of globs) {
    const path = glob;
    if (path.length > 0) {
      if (!joined) joined = path;
      else joined += `${SEP}${path}`;
    }
  }
  if (!joined) return ".";
  return normalizeGlob(joined, { extended, globstar });
}

export interface ExpandGlobOptions extends GlobOptions {
  root?: string;
  exclude?: string[];
  includeDirs?: boolean;
}

interface SplitPath {
  segments: string[];
  isAbsolute: boolean;
  hasTrailingSep: boolean;
  // Defined for any absolute Windows path.
  winRoot?: string;
}

// TODO: Maybe make this public somewhere.
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
    winRoot: isWindows && isAbsolute_ ? segments.shift() : undefined
  };
}

/**
 * Expand the glob string from the specified `root` directory and yield each
 * result as a `WalkInfo` object.
 */
// TODO: Use a proper glob expansion algorithm.
// This is a very incomplete solution. The whole directory tree from `root` is
// walked and parent paths are not supported.
export async function* expandGlob(
  glob: string,
  {
    root = cwd(),
    exclude = [],
    includeDirs = true,
    extended = false,
    globstar = false
  }: ExpandGlobOptions = {}
): AsyncIterableIterator<WalkInfo> {
  const globOptions: GlobOptions = { extended, globstar };
  const absRoot = isAbsolute(root)
    ? normalize(root)
    : joinGlobs([cwd(), root], globOptions);
  const resolveFromRoot = (path: string): string =>
    isAbsolute(path)
      ? normalize(path)
      : joinGlobs([absRoot, path], globOptions);
  const excludePatterns = exclude
    .map(resolveFromRoot)
    .map((s: string): RegExp => globToRegExp(s, globOptions));
  const shouldInclude = ({ filename }: WalkInfo): boolean =>
    !excludePatterns.some((p: RegExp): boolean => !!filename.match(p));
  const { segments, hasTrailingSep, winRoot } = split(resolveFromRoot(glob));

  let fixedRoot = winRoot != undefined ? winRoot : "/";
  while (segments.length > 0 && !isGlob(segments[0])) {
    fixedRoot = joinGlobs([fixedRoot, segments.shift()!], globOptions);
  }

  let fixedRootInfo: WalkInfo;
  try {
    fixedRootInfo = { filename: fixedRoot, info: await stat(fixedRoot) };
  } catch {
    return;
  }

  async function* advanceMatch(
    walkInfo: WalkInfo,
    globSegment: string
  ): AsyncIterableIterator<WalkInfo> {
    if (!walkInfo.info.isDirectory()) {
      return;
    } else if (globSegment == "..") {
      const parentPath = joinGlobs([walkInfo.filename, ".."], globOptions);
      try {
        return yield* [
          { filename: parentPath, info: await stat(parentPath) }
        ].filter(shouldInclude);
      } catch {
        return;
      }
    } else if (globSegment == "**") {
      return yield* walk(walkInfo.filename, {
        includeFiles: false,
        skip: excludePatterns
      });
    }
    yield* walk(walkInfo.filename, {
      maxDepth: 1,
      match: [
        globToRegExp(
          joinGlobs([walkInfo.filename, globSegment], globOptions),
          globOptions
        )
      ],
      skip: excludePatterns
    });
  }

  let currentMatches: WalkInfo[] = [fixedRootInfo];
  for (const segment of segments) {
    // Advancing the list of current matches may introduce duplicates, so we
    // pass everything through this Map.
    const nextMatchMap: Map<string, FileInfo> = new Map();
    for (const currentMatch of currentMatches) {
      for await (const nextMatch of advanceMatch(currentMatch, segment)) {
        nextMatchMap.set(nextMatch.filename, nextMatch.info);
      }
    }
    currentMatches = [...nextMatchMap].sort().map(
      ([filename, info]): WalkInfo => ({
        filename,
        info
      })
    );
  }
  if (hasTrailingSep) {
    currentMatches = currentMatches.filter(({ info }): boolean =>
      info.isDirectory()
    );
  }
  if (!includeDirs) {
    currentMatches = currentMatches.filter(
      ({ info }): boolean => !info.isDirectory()
    );
  }
  yield* currentMatches;
}

/** Synchronous version of `expandGlob()`. */
// TODO: As `expandGlob()`.
export function* expandGlobSync(
  glob: string,
  {
    root = cwd(),
    exclude = [],
    includeDirs = true,
    extended = false,
    globstar = false
  }: ExpandGlobOptions = {}
): IterableIterator<WalkInfo> {
  const globOptions: GlobOptions = { extended, globstar };
  const absRoot = isAbsolute(root)
    ? normalize(root)
    : joinGlobs([cwd(), root], globOptions);
  const resolveFromRoot = (path: string): string =>
    isAbsolute(path)
      ? normalize(path)
      : joinGlobs([absRoot, path], globOptions);
  const excludePatterns = exclude
    .map(resolveFromRoot)
    .map((s: string): RegExp => globToRegExp(s, globOptions));
  const shouldInclude = ({ filename }: WalkInfo): boolean =>
    !excludePatterns.some((p: RegExp): boolean => !!filename.match(p));
  const { segments, hasTrailingSep, winRoot } = split(resolveFromRoot(glob));

  let fixedRoot = winRoot != undefined ? winRoot : "/";
  while (segments.length > 0 && !isGlob(segments[0])) {
    fixedRoot = joinGlobs([fixedRoot, segments.shift()!], globOptions);
  }

  let fixedRootInfo: WalkInfo;
  try {
    fixedRootInfo = { filename: fixedRoot, info: statSync(fixedRoot) };
  } catch {
    return;
  }

  function* advanceMatch(
    walkInfo: WalkInfo,
    globSegment: string
  ): IterableIterator<WalkInfo> {
    if (!walkInfo.info.isDirectory()) {
      return;
    } else if (globSegment == "..") {
      const parentPath = joinGlobs([walkInfo.filename, ".."], globOptions);
      try {
        return yield* [
          { filename: parentPath, info: statSync(parentPath) }
        ].filter(shouldInclude);
      } catch {
        return;
      }
    } else if (globSegment == "**") {
      return yield* walkSync(walkInfo.filename, {
        includeFiles: false,
        skip: excludePatterns
      });
    }
    yield* walkSync(walkInfo.filename, {
      maxDepth: 1,
      match: [
        globToRegExp(
          joinGlobs([walkInfo.filename, globSegment], globOptions),
          globOptions
        )
      ],
      skip: excludePatterns
    });
  }

  let currentMatches: WalkInfo[] = [fixedRootInfo];
  for (const segment of segments) {
    // Advancing the list of current matches may introduce duplicates, so we
    // pass everything through this Map.
    const nextMatchMap: Map<string, FileInfo> = new Map();
    for (const currentMatch of currentMatches) {
      for (const nextMatch of advanceMatch(currentMatch, segment)) {
        nextMatchMap.set(nextMatch.filename, nextMatch.info);
      }
    }
    currentMatches = [...nextMatchMap].sort().map(
      ([filename, info]): WalkInfo => ({
        filename,
        info
      })
    );
  }
  if (hasTrailingSep) {
    currentMatches = currentMatches.filter(({ info }): boolean =>
      info.isDirectory()
    );
  }
  if (!includeDirs) {
    currentMatches = currentMatches.filter(
      ({ info }): boolean => !info.isDirectory()
    );
  }
  yield* currentMatches;
}
