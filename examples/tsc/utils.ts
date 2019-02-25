/* These are directly adapted from TypeScript and are licensed as: */

/*! ****************************************************************************
Copyright (c) Microsoft Corporation. All rights reserved. 
Licensed under the Apache License, Version 2.0 (the "License"); you may not use
this file except in compliance with the License. You may obtain a copy of the
License at http://www.apache.org/licenses/LICENSE-2.0  
 
THIS CODE IS PROVIDED ON AN *AS IS* BASIS, WITHOUT WARRANTIES OR CONDITIONS OF
ANY KIND, EITHER EXPRESS OR IMPLIED, INCLUDING WITHOUT LIMITATION ANY IMPLIED
WARRANTIES OR CONDITIONS OF TITLE, FITNESS FOR A PARTICULAR PURPOSE, 
MERCHANTABLITY OR NON-INFRINGEMENT. 
 
See the Apache Version 2.0 License for specific language governing permissions
and limitations under the License.
***************************************************************************** */

import * as dir from "../../js/dir";
import { readDirSync } from "../../js/read_dir";
import { statSync } from "../../js/stat";
import { assert } from "../../js/util";

const CHAR_UPPERCASE_A = 65;
const CHAR_LOWERCASE_A = 97;
const CHAR_UPPERCASE_Z = 90;
const CHAR_LOWERCASE_Z = 122;
const CHAR_DOT = 46;
const CHAR_FORWARD_SLASH = 47;
const CHAR_BACKWARD_SLASH = 92;
const CHAR_COLON = 58;
const CHAR_PERCENT = 37;
const CHAR_QUESTION_MARK = 63;
const CHAR_ASTERISK = 0x2a;
const CHAR_3 = 0x33;

export const enum FileSystemEntryKind {
  File,
  Directory
}

export function fileSystemEntryExists(
  path: string,
  entryKind: FileSystemEntryKind
): boolean {
  try {
    const stat = statSync(path);
    switch (entryKind) {
      case FileSystemEntryKind.File:
        return stat.isFile();
      case FileSystemEntryKind.Directory:
        return stat.isDirectory();
      default:
        return false;
    }
  } catch (e) {
    return false;
  }
}

function isPosixPathSeparator(code: number): boolean {
  return code === CHAR_FORWARD_SLASH;
}

function normalizeString(
  path: string,
  allowAboveRoot: boolean,
  separator: string,
  isPathSeparator: (code: number) => boolean
): string {
  let res = "";
  let lastSegmentLength = 0;
  let lastSlash = -1;
  let dots = 0;
  let code: number;
  for (let i = 0, len = path.length; i <= len; ++i) {
    if (i < len) {
      code = path.charCodeAt(i);
    } else if (isPathSeparator(code!)) {
      break;
    } else {
      code = CHAR_FORWARD_SLASH;
    }

    if (isPathSeparator(code)) {
      if (lastSlash === i - 1 || dots === 1) {
        // NOOP
      } else if (lastSlash !== i - 1 && dots === 2) {
        if (
          res.length < 2 ||
          lastSegmentLength !== 2 ||
          res.charCodeAt(res.length - 1) !== CHAR_DOT ||
          res.charCodeAt(res.length - 2) !== CHAR_DOT
        ) {
          if (res.length > 2) {
            const lastSlashIndex = res.lastIndexOf(separator);
            if (lastSlashIndex === -1) {
              res = "";
              lastSegmentLength = 0;
            } else {
              res = res.slice(0, lastSlashIndex);
              lastSegmentLength = res.length - 1 - res.lastIndexOf(separator);
            }
            lastSlash = i;
            dots = 0;
            continue;
          } else if (res.length === 2 || res.length === 1) {
            res = "";
            lastSegmentLength = 0;
            lastSlash = i;
            dots = 0;
            continue;
          }
        }
        if (allowAboveRoot) {
          if (res.length > 0) {
            res += `${separator}..`;
          } else {
            res = "..";
          }
          lastSegmentLength = 2;
        }
      } else {
        if (res.length > 0) {
          res += separator + path.slice(lastSlash + 1, i);
        } else {
          res = path.slice(lastSlash + 1, i);
        }
        lastSegmentLength = i - lastSlash - 1;
      }
      lastSlash = i;
      dots = 0;
    } else if (code === CHAR_DOT && dots !== -1) {
      ++dots;
    } else {
      dots = -1;
    }
  }
  return res;
}

export function resolve(...pathSegments: string[]): string {
  let resolvedPath = "";
  let resolvedAbsolute = false;

  for (let i = pathSegments.length - 1; i >= -1 && !resolvedAbsolute; i--) {
    let path: string;

    if (i >= 0) {
      path = pathSegments[i];
    } else {
      path = dir.cwd();
    }

    // Skip empty entries
    if (path.length === 0) {
      continue;
    }

    resolvedPath = `${path}/${resolvedPath}`;
    resolvedAbsolute = path.charCodeAt(0) === CHAR_FORWARD_SLASH;
  }

  // At this point the path should be resolved to a full absolute path, but
  // handle relative paths to be safe (might happen when process.cwd() fails)

  // Normalize the path
  resolvedPath = normalizeString(
    resolvedPath,
    !resolvedAbsolute,
    "/",
    isPosixPathSeparator
  );

  if (resolvedAbsolute) {
    if (resolvedPath.length > 0) {
      return `/${resolvedPath}`;
    } else {
      return "/";
    }
  } else if (resolvedPath.length > 0) {
    return resolvedPath;
  } else {
    return ".";
  }
}

const directorySeparator = "/";
const altDirectorySeparator = "\\";
const urlSchemeSeparator = "://";
const backslashRegExp = /\\/g;

function normalizeSlashes(path: string): string {
  return path.replace(backslashRegExp, directorySeparator);
}

function isVolumeCharacter(charCode: number) {
  return (
    (charCode >= CHAR_LOWERCASE_A && charCode <= CHAR_LOWERCASE_Z) ||
    (charCode >= CHAR_UPPERCASE_A && charCode <= CHAR_UPPERCASE_Z)
  );
}

function getFileUrlVolumeSeparatorEnd(url: string, start: number) {
  const ch0 = url.charCodeAt(start);
  if (ch0 === CHAR_COLON) {
    return start + 1;
  }
  if (ch0 === CHAR_PERCENT && url.charCodeAt(start + 1) === CHAR_3) {
    const ch2 = url.charCodeAt(start + 2);
    if (ch2 === CHAR_LOWERCASE_A || ch2 === CHAR_UPPERCASE_A) {
      return start + 3;
    }
  }
  return -1;
}

function getEncodedRootLength(path: string): number {
  if (!path) {
    return 0;
  }
  const ch0 = path.charCodeAt(0);

  // POSIX or UNC
  if (ch0 === CHAR_FORWARD_SLASH || ch0 === CHAR_BACKWARD_SLASH) {
    if (path.charCodeAt(1) !== ch0) {
      return 1; // POSIX: "/" (or non-normalized "\")
    }

    const p1 = path.indexOf(
      ch0 === CHAR_FORWARD_SLASH ? directorySeparator : altDirectorySeparator,
      2
    );
    if (p1 < 0) {
      return path.length; // UNC: "//server" or "\\server"
    }

    return p1 + 1; // UNC: "//server/" or "\\server\"
  }

  // DOS
  if (isVolumeCharacter(ch0) && path.charCodeAt(1) === CHAR_COLON) {
    const ch2 = path.charCodeAt(2);
    if (ch2 === CHAR_FORWARD_SLASH || ch2 === CHAR_BACKWARD_SLASH) {
      return 3; // DOS: "c:/" or "c:\"
    }
    if (path.length === 2) {
      return 2; // DOS: "c:" (but not "c:d")
    }
  }

  // URL
  const schemeEnd = path.indexOf(urlSchemeSeparator);
  if (schemeEnd !== -1) {
    const authorityStart = schemeEnd + urlSchemeSeparator.length;
    const authorityEnd = path.indexOf(directorySeparator, authorityStart);
    if (authorityEnd !== -1) {
      // URL: "file:///", "file://server/", "file://server/path"
      // For local "file" URLs, include the leading DOS volume (if present).
      // Per https://www.ietf.org/rfc/rfc1738.txt, a host of "" or "localhost"
      // is a special case interpreted as "the machine from which the URL is
      // being interpreted".
      const scheme = path.slice(0, schemeEnd);
      const authority = path.slice(authorityStart, authorityEnd);
      if (
        scheme === "file" &&
        (authority === "" || authority === "localhost") &&
        isVolumeCharacter(path.charCodeAt(authorityEnd + 1))
      ) {
        const volumeSeparatorEnd = getFileUrlVolumeSeparatorEnd(
          path,
          authorityEnd + 2
        );
        if (volumeSeparatorEnd !== -1) {
          if (path.charCodeAt(volumeSeparatorEnd) === CHAR_FORWARD_SLASH) {
            return ~(volumeSeparatorEnd + 1);
          }
          if (volumeSeparatorEnd === path.length) {
            return ~volumeSeparatorEnd;
          }
        }
      }
      return ~(authorityEnd + 1); // URL: "file://server/", "http://server/"
    }
    return ~path.length; // URL: "file://server", "http://server"
  }

  // relative
  return 0;
}

function getRootLength(path: string) {
  const rootLength = getEncodedRootLength(path);
  return rootLength < 0 ? ~rootLength : rootLength;
}

function hasTrailingDirectorySeparator(path: string) {
  if (path.length === 0) {
    return false;
  }
  const ch = path.charCodeAt(path.length - 1);
  return ch === CHAR_FORWARD_SLASH || ch === CHAR_BACKWARD_SLASH;
}

function ensureTrailingDirectorySeparator(path: string) {
  if (!hasTrailingDirectorySeparator(path)) {
    return path + directorySeparator;
  }

  return path;
}

export function combinePaths(
  path: string,
  ...paths: Array<string | undefined>
): string {
  if (path) {
    path = normalizeSlashes(path);
  }
  for (let relativePath of paths) {
    if (!relativePath) {
      continue;
    }
    relativePath = normalizeSlashes(relativePath);
    if (!path || getRootLength(relativePath) !== 0) {
      path = relativePath;
    } else {
      path = ensureTrailingDirectorySeparator(path) + relativePath;
    }
  }
  return path;
}

function getPathFromPathComponents(pathComponents: ReadonlyArray<string>) {
  if (pathComponents.length === 0) {
    return "";
  }

  const root =
    pathComponents[0] && ensureTrailingDirectorySeparator(pathComponents[0]);
  return root + pathComponents.slice(1).join(directorySeparator);
}

function reducePathComponents(components: ReadonlyArray<string>) {
  if (!components.length) {
    return [];
  }
  const reduced = [components[0]];
  for (let i = 1; i < components.length; i++) {
    const component = components[i];
    if (!component) {
      continue;
    }
    if (component === ".") {
      continue;
    }
    if (component === "..") {
      if (reduced.length > 1) {
        if (reduced[reduced.length - 1] !== "..") {
          reduced.pop();
          continue;
        }
      } else if (reduced[0]) {
        continue;
      }
    }
    reduced.push(component);
  }
  return reduced;
}

function lastOrUndefined<T>(array: ReadonlyArray<T>): T | undefined {
  return array.length === 0 ? undefined : array[array.length - 1];
}

function pathComponents(path: string, rootLength: number) {
  const root = path.substring(0, rootLength);
  const rest = path.substring(rootLength).split(directorySeparator);
  if (rest.length && !lastOrUndefined(rest)) {
    rest.pop();
  }
  return [root, ...rest];
}

function getPathComponents(path: string, currentDirectory = "") {
  path = combinePaths(currentDirectory, path);
  const rootLength = getRootLength(path);
  return pathComponents(path, rootLength);
}

export function resolvePath(
  path: string,
  ...paths: Array<string | undefined>
): string {
  const combined = paths.length
    ? combinePaths(path, ...paths)
    : normalizeSlashes(path);
  const normalized = getPathFromPathComponents(
    reducePathComponents(getPathComponents(combined))
  );
  return normalized && hasTrailingDirectorySeparator(combined)
    ? ensureTrailingDirectorySeparator(normalized)
    : normalized;
}

interface FileSystemEntries {
  readonly files: ReadonlyArray<string>;
  readonly directories: ReadonlyArray<string>;
}

interface WildcardMatcher {
  singleAsteriskRegexFragment: string;
  doubleAsteriskRegexFragment: string;
  replaceWildcardCharacter: (match: string) => string;
}

const commonPackageFolders: ReadonlyArray<string> = [
  "node_modules",
  "bower_components",
  "jspm_packages"
];

const implicitExcludePathRegexPattern = `(?!(${commonPackageFolders.join(
  "|"
)})(/|$))`;

function replaceWildcardCharacter(
  match: string,
  singleAsteriskRegexFragment: string
) {
  return match === "*"
    ? singleAsteriskRegexFragment
    : match === "?"
    ? "[^/]"
    : "\\" + match;
}

const filesMatcher: WildcardMatcher = {
  singleAsteriskRegexFragment: "([^./]|(\\.(?!min\\.js$))?)*",
  // tslint:disable-next-line:max-line-length
  doubleAsteriskRegexFragment: `(/${implicitExcludePathRegexPattern}[^/.][^/]*)*?`,
  replaceWildcardCharacter: match =>
    replaceWildcardCharacter(match, filesMatcher.singleAsteriskRegexFragment)
};

const directoriesMatcher: WildcardMatcher = {
  singleAsteriskRegexFragment: "[^/]*",
  // tslint:disable-next-line:max-line-length
  doubleAsteriskRegexFragment: `(/${implicitExcludePathRegexPattern}[^/.][^/]*)*?`,
  replaceWildcardCharacter: match =>
    replaceWildcardCharacter(
      match,
      directoriesMatcher.singleAsteriskRegexFragment
    )
};

const excludeMatcher: WildcardMatcher = {
  singleAsteriskRegexFragment: "[^/]*",
  doubleAsteriskRegexFragment: "(/.+?)?",
  replaceWildcardCharacter: match =>
    replaceWildcardCharacter(match, excludeMatcher.singleAsteriskRegexFragment)
};

const wildcardMatchers = {
  files: filesMatcher,
  directories: directoriesMatcher,
  exclude: excludeMatcher
};

function getNormalizedPathComponents(
  path: string,
  currentDirectory: string | undefined
) {
  return reducePathComponents(getPathComponents(path, currentDirectory));
}

function last<T>(array: ReadonlyArray<T>): T {
  assert(array.length !== 0);
  return array[array.length - 1];
}

function removeTrailingDirectorySeparator(path: string) {
  if (hasTrailingDirectorySeparator(path)) {
    return path.substr(0, path.length - 1);
  }

  return path;
}

function isImplicitGlob(lastPathComponent: string): boolean {
  return !/[.*?]/.test(lastPathComponent);
}

const reservedCharacterPattern = /[^\w\s\/]/g;

function getSubPatternFromSpec(
  spec: string,
  basePath: string,
  usage: "files" | "directories" | "exclude",
  {
    singleAsteriskRegexFragment,
    doubleAsteriskRegexFragment,
    replaceWildcardCharacter
  }: WildcardMatcher
): string | undefined {
  let subpattern = "";
  let hasWrittenComponent = false;
  const components = getNormalizedPathComponents(spec, basePath);
  const lastComponent = last(components);
  if (usage !== "exclude" && lastComponent === "**") {
    return undefined;
  }

  // getNormalizedPathComponents includes the separator for the root component.
  // We need to remove to create our regex correctly.
  components[0] = removeTrailingDirectorySeparator(components[0]);

  if (isImplicitGlob(lastComponent)) {
    components.push("**", "*");
  }

  let optionalCount = 0;
  for (let component of components) {
    if (component === "**") {
      subpattern += doubleAsteriskRegexFragment;
    } else {
      if (usage === "directories") {
        subpattern += "(";
        optionalCount++;
      }

      if (hasWrittenComponent) {
        subpattern += directorySeparator;
      }

      if (usage !== "exclude") {
        let componentPattern = "";
        if (component.charCodeAt(0) === CHAR_ASTERISK) {
          componentPattern += "([^./]" + singleAsteriskRegexFragment + ")?";
          component = component.substr(1);
        } else if (component.charCodeAt(0) === CHAR_QUESTION_MARK) {
          componentPattern += "[^./]";
          component = component.substr(1);
        }

        componentPattern += component.replace(
          reservedCharacterPattern,
          replaceWildcardCharacter
        );

        if (componentPattern !== component) {
          subpattern += implicitExcludePathRegexPattern;
        }

        subpattern += componentPattern;
      } else {
        subpattern += component.replace(
          reservedCharacterPattern,
          replaceWildcardCharacter
        );
      }
    }

    hasWrittenComponent = true;
  }

  while (optionalCount > 0) {
    subpattern += ")?";
    optionalCount--;
  }

  return subpattern;
}

function getRegularExpressionsForWildcards(
  specs: ReadonlyArray<string> | undefined,
  basePath: string,
  usage: "files" | "directories" | "exclude"
): ReadonlyArray<string> | undefined {
  if (specs === undefined || specs.length === 0) {
    return undefined;
  }

  return (
    specs &&
    specs.flatMap(
      spec =>
        spec &&
        getSubPatternFromSpec(spec, basePath, usage, wildcardMatchers[usage])!
    )
  );
}

interface FileMatcherPatterns {
  /** One pattern for each "include" spec. */
  includeFilePatterns: ReadonlyArray<string> | undefined;
  /** One pattern matching one of any of the "include" specs. */
  includeFilePattern: string | undefined;
  includeDirectoryPattern: string | undefined;
  excludePattern: string | undefined;
  basePaths: ReadonlyArray<string>;
}

function getRegularExpressionForWildcard(
  specs: ReadonlyArray<string> | undefined,
  basePath: string,
  usage: "files" | "directories" | "exclude"
): string | undefined {
  const patterns = getRegularExpressionsForWildcards(specs, basePath, usage);
  if (!patterns || !patterns.length) {
    return undefined;
  }

  const pattern = patterns.map(pattern => `(${pattern})`).join("|");
  // If excluding, match "foo/bar/baz...", but if including, only allow "foo".
  const terminator = usage === "exclude" ? "($|/)" : "$";
  return `^(${pattern})${terminator}`;
}

function isRootedDiskPath(path: string) {
  return getEncodedRootLength(path) > 0;
}

function getAnyExtensionFromPathWorker(
  path: string,
  extensions: string | ReadonlyArray<string>,
  stringEqualityComparer: (a: string, b: string) => boolean
) {
  if (typeof extensions === "string") {
    extensions = [extensions];
  }
  for (let extension of extensions) {
    if (!extension.startsWith(".")) {
      extension = "." + extension;
    }
    if (
      path.length >= extension.length &&
      path.charAt(path.length - extension.length) === "."
    ) {
      const pathExtension = path.slice(path.length - extension.length);
      if (stringEqualityComparer(pathExtension, extension)) {
        return pathExtension;
      }
    }
  }
  return "";
}

function equateValues<T>(a: T, b: T) {
  return a === b;
}

function equateStringsCaseInsensitive(a: string, b: string) {
  return (
    a === b ||
    (a !== undefined && b !== undefined && a.toUpperCase() === b.toUpperCase())
  );
}

function equateStringsCaseSensitive(a: string, b: string) {
  return equateValues(a, b);
}

function getAnyExtensionFromPath(path: string): string;
function getAnyExtensionFromPath(
  path: string,
  extensions: string | ReadonlyArray<string>,
  ignoreCase: boolean
): string;
function getAnyExtensionFromPath(
  path: string,
  extensions?: string | ReadonlyArray<string>,
  ignoreCase?: boolean
): string {
  if (extensions) {
    return getAnyExtensionFromPathWorker(
      path,
      extensions,
      ignoreCase ? equateStringsCaseInsensitive : equateStringsCaseSensitive
    );
  }
  const baseFileName = getBaseFileName(path);
  const extensionIndex = baseFileName.lastIndexOf(".");
  if (extensionIndex >= 0) {
    return baseFileName.substring(extensionIndex);
  }
  return "";
}

function getBaseFileName(path: string): string;
function getBaseFileName(
  path: string,
  extensions: string | ReadonlyArray<string>,
  ignoreCase: boolean
): string;
function getBaseFileName(
  path: string,
  extensions?: string | ReadonlyArray<string>,
  ignoreCase?: boolean
) {
  path = normalizeSlashes(path);

  // if the path provided is itself the root, then it has not file name.
  const rootLength = getRootLength(path);
  if (rootLength === path.length) {
    return "";
  }

  path = removeTrailingDirectorySeparator(path);
  const name = path.slice(
    Math.max(getRootLength(path), path.lastIndexOf(directorySeparator) + 1)
  );
  const extension =
    extensions !== undefined && ignoreCase !== undefined
      ? getAnyExtensionFromPath(name, extensions, ignoreCase)
      : undefined;
  return extension ? name.slice(0, name.length - extension.length) : name;
}

function hasExtension(fileName: string): boolean {
  return getBaseFileName(fileName).includes(".");
}

function indexOfAnyCharCode(
  text: string,
  charCodes: ReadonlyArray<number>,
  start?: number
): number {
  for (let i = start || 0; i < text.length; i++) {
    if (charCodes.includes(text.charCodeAt(i))) {
      return i;
    }
  }
  return -1;
}

function getDirectoryPath(path: string): string {
  path = normalizeSlashes(path);

  // If the path provided is itself the root, then return it.
  const rootLength = getRootLength(path);
  if (rootLength === path.length) {
    return path;
  }

  path = removeTrailingDirectorySeparator(path);
  return path.slice(
    0,
    Math.max(rootLength, path.lastIndexOf(directorySeparator))
  );
}

const wildcardCharCodes = [CHAR_ASTERISK, CHAR_QUESTION_MARK];

function getIncludeBasePath(absolute: string): string {
  const wildcardOffset = indexOfAnyCharCode(absolute, wildcardCharCodes);
  if (wildcardOffset < 0) {
    // No "*" or "?" in the path
    return !hasExtension(absolute)
      ? absolute
      : removeTrailingDirectorySeparator(getDirectoryPath(absolute));
  }
  return absolute.substring(
    0,
    absolute.lastIndexOf(directorySeparator, wildcardOffset)
  );
}

function containsPath(
  parent: string,
  child: string,
  ignoreCase?: boolean
): boolean;
function containsPath(
  parent: string,
  child: string,
  currentDirectory: string,
  ignoreCase?: boolean
): boolean;
function containsPath(
  parent: string,
  child: string,
  currentDirectory?: string | boolean,
  ignoreCase?: boolean
) {
  if (typeof currentDirectory === "string") {
    parent = combinePaths(currentDirectory, parent);
    child = combinePaths(currentDirectory, child);
  } else if (typeof currentDirectory === "boolean") {
    ignoreCase = currentDirectory;
  }
  if (parent === undefined || child === undefined) {
    return false;
  }
  if (parent === child) {
    return true;
  }
  const parentComponents = reducePathComponents(getPathComponents(parent));
  const childComponents = reducePathComponents(getPathComponents(child));
  if (childComponents.length < parentComponents.length) {
    return false;
  }

  const componentEqualityComparer = ignoreCase
    ? equateStringsCaseInsensitive
    : equateStringsCaseSensitive;
  for (let i = 0; i < parentComponents.length; i++) {
    const equalityComparer =
      i === 0 ? equateStringsCaseInsensitive : componentEqualityComparer;
    if (!equalityComparer(parentComponents[i], childComponents[i])) {
      return false;
    }
  }

  return true;
}

const enum Comparison {
  LessThan = -1,
  EqualTo = 0,
  GreaterThan = 1
}

function compareStringsCaseInsensitive(a: string, b: string) {
  if (a === b) {
    return Comparison.EqualTo;
  }
  if (a === undefined) {
    return Comparison.LessThan;
  }
  if (b === undefined) {
    return Comparison.GreaterThan;
  }
  a = a.toUpperCase();
  b = b.toUpperCase();
  return a < b
    ? Comparison.LessThan
    : a > b
    ? Comparison.GreaterThan
    : Comparison.EqualTo;
}

function compareComparableValues(
  a: string | undefined,
  b: string | undefined
): Comparison;
function compareComparableValues(
  a: number | undefined,
  b: number | undefined
): Comparison;
function compareComparableValues(
  a: string | number | undefined,
  b: string | number | undefined
) {
  return a === b
    ? Comparison.EqualTo
    : a === undefined
    ? Comparison.LessThan
    : b === undefined
    ? Comparison.GreaterThan
    : a < b
    ? Comparison.LessThan
    : Comparison.GreaterThan;
}

function compareStringsCaseSensitive(
  a: string | undefined,
  b: string | undefined
): Comparison {
  return compareComparableValues(a, b);
}

function getStringComparer(ignoreCase?: boolean) {
  return ignoreCase
    ? compareStringsCaseInsensitive
    : compareStringsCaseSensitive;
}

function getBasePaths(
  path: string,
  includes: ReadonlyArray<string> | undefined,
  useCaseSensitiveFileNames: boolean
): string[] {
  const basePaths: string[] = [path];

  if (includes) {
    // Storage for literal base paths amongst the include patterns.
    const includeBasePaths: string[] = [];
    for (const include of includes) {
      const absolute: string = isRootedDiskPath(include)
        ? include
        : resolvePath(combinePaths(path, include));
      // Append the literal and canonical candidate base paths.
      includeBasePaths.push(getIncludeBasePath(absolute));
    }

    includeBasePaths.sort(getStringComparer(!useCaseSensitiveFileNames));

    for (const includeBasePath of includeBasePaths) {
      if (
        basePaths.every(
          basePath =>
            !containsPath(
              basePath,
              includeBasePath,
              path,
              !useCaseSensitiveFileNames
            )
        )
      ) {
        basePaths.push(includeBasePath);
      }
    }
  }

  return basePaths;
}

function getFileMatcherPatterns(
  path: string,
  excludes: ReadonlyArray<string> | undefined,
  includes: ReadonlyArray<string> | undefined,
  useCaseSensitiveFileNames: boolean,
  currentDirectory: string
): FileMatcherPatterns {
  path = resolvePath(path);
  currentDirectory = resolvePath(currentDirectory);
  const absolutePath = combinePaths(currentDirectory, path);

  return {
    includeFilePatterns: getRegularExpressionsForWildcards(
      includes,
      absolutePath,
      "files"
    )!.map(pattern => `^${pattern}$`),
    includeFilePattern: getRegularExpressionForWildcard(
      includes,
      absolutePath,
      "files"
    ),
    includeDirectoryPattern: getRegularExpressionForWildcard(
      includes,
      absolutePath,
      "directories"
    ),
    excludePattern: getRegularExpressionForWildcard(
      excludes,
      absolutePath,
      "exclude"
    ),
    basePaths: getBasePaths(path, includes, useCaseSensitiveFileNames)
  };
}

function getRegexFromPattern(
  pattern: string,
  useCaseSensitiveFileNames: boolean
): RegExp {
  return new RegExp(pattern, useCaseSensitiveFileNames ? "" : "i");
}

function fileExtensionIs(path: string, extension: string): boolean {
  return path.length > extension.length && path.endsWith(extension);
}

function fileExtensionIsOneOf(
  path: string,
  extensions: ReadonlyArray<string>
): boolean {
  for (const extension of extensions) {
    if (fileExtensionIs(path, extension)) {
      return true;
    }
  }

  return false;
}

export function matchFiles(
  path: string,
  extensions: ReadonlyArray<string> | undefined,
  excludes: ReadonlyArray<string> | undefined,
  includes: ReadonlyArray<string> | undefined,
  useCaseSensitiveFileNames: boolean,
  currentDirectory: string,
  depth: number | undefined,
  getFileSystemEntries: (path: string) => FileSystemEntries
): string[] {
  path = resolve(path);
  currentDirectory = resolvePath(currentDirectory);

  const patterns = getFileMatcherPatterns(
    path,
    excludes,
    includes,
    useCaseSensitiveFileNames,
    currentDirectory
  );

  const includeFileRegexes =
    patterns.includeFilePatterns &&
    patterns.includeFilePatterns.map(pattern =>
      getRegexFromPattern(pattern, useCaseSensitiveFileNames)
    );
  const includeDirectoryRegex =
    patterns.includeDirectoryPattern &&
    getRegexFromPattern(
      patterns.includeDirectoryPattern,
      useCaseSensitiveFileNames
    );
  const excludeRegex =
    patterns.excludePattern &&
    getRegexFromPattern(patterns.excludePattern, useCaseSensitiveFileNames);

  const results: string[][] = includeFileRegexes
    ? includeFileRegexes.map(() => [])
    : [[]];

  for (const basePath of patterns.basePaths) {
    visitDirectory(basePath, combinePaths(currentDirectory, basePath), depth);
  }

  return results.flat();

  function visitDirectory(
    path: string,
    absolutePath: string,
    depth: number | undefined
  ) {
    const { files, directories } = getFileSystemEntries(path);

    for (const current of files.slice(0).sort(compareStringsCaseSensitive)) {
      const name = combinePaths(path, current);
      const absoluteName = combinePaths(absolutePath, current);
      if (extensions && !fileExtensionIsOneOf(name, extensions)) {
        continue;
      }
      if (excludeRegex && excludeRegex.test(absoluteName)) {
        continue;
      }
      if (!includeFileRegexes) {
        results[0].push(name);
      } else {
        const includeIndex = includeFileRegexes.findIndex(re =>
          re.test(absoluteName)
        );
        if (includeIndex !== -1) {
          results[includeIndex].push(name);
        }
      }
    }

    if (depth !== undefined) {
      depth--;
      if (depth === 0) {
        return;
      }
    }

    for (const current of directories
      .slice(0)
      .sort(compareStringsCaseSensitive)) {
      const name = combinePaths(path, current);
      const absoluteName = combinePaths(absolutePath, current);
      if (
        (!includeDirectoryRegex || includeDirectoryRegex.test(absoluteName)) &&
        (!excludeRegex || !excludeRegex.test(absoluteName))
      ) {
        visitDirectory(name, absoluteName, depth);
      }
    }
  }
}

const emptyArray: never[] = [];

const emptyFileSystemEntries: FileSystemEntries = {
  files: emptyArray,
  directories: emptyArray
};

export function getAccessibleFileSystemEntries(
  path: string
): FileSystemEntries {
  try {
    const entries = readDirSync(path || ".").sort();
    const files: string[] = [];
    const directories: string[] = [];
    for (const entry of entries.map(entry => entry.name)) {
      // This is necessary because on some file system node fails to exclude
      // "." and "..". See https://github.com/nodejs/node/issues/4002
      if (entry === "." || entry === ".." || entry === null) {
        continue;
      }
      const name = combinePaths(path, entry);

      let stat: any; // tslint:disable-line:no-any
      try {
        stat = statSync(name);
      } catch (e) {
        continue;
      }

      if (stat.isFile()) {
        files.push(entry);
      } else if (stat.isDirectory()) {
        directories.push(entry);
      }
    }
    return { files, directories };
  } catch (e) {
    return emptyFileSystemEntries;
  }
}

function forEachAncestorDirectory<T>(
  directory: string,
  callback: (directory: string) => T | undefined
): T | undefined {
  while (true) {
    const result = callback(directory);
    if (result !== undefined) {
      return result;
    }

    const parentPath = getDirectoryPath(directory);
    if (parentPath === directory) {
      return undefined;
    }

    directory = parentPath;
  }
}

export function findConfigFile(
  searchPath: string,
  fileExists: (fileName: string) => boolean,
  configName = "tsconfig.json"
): string | undefined {
  return forEachAncestorDirectory(searchPath, ancestor => {
    const fileName = combinePaths(ancestor, configName);
    return fileExists(fileName) ? fileName : undefined;
  });
}
