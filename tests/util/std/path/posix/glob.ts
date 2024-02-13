// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

export {
  /**
   * @deprecated (will be removed in 0.215.0) Import from {@link https://deno.land/std/path/posix/is_glob.ts} instead.
   *
   * Test whether the given string is a glob
   */
  isGlob,
} from "../is_glob.ts";

export {
  /**
   * @deprecated (will be removed in 0.215.0) Import from {@link https://deno.land/std/path/posix/glob_to_regexp.ts} instead.
   *
   * Convert a glob string to a regular expression.
   *
   * Tries to match bash glob expansion as closely as possible.
   *
   * Basic glob syntax:
   * - `*` - Matches everything without leaving the path segment.
   * - `?` - Matches any single character.
   * - `{foo,bar}` - Matches `foo` or `bar`.
   * - `[abcd]` - Matches `a`, `b`, `c` or `d`.
   * - `[a-d]` - Matches `a`, `b`, `c` or `d`.
   * - `[!abcd]` - Matches any single character besides `a`, `b`, `c` or `d`.
   * - `[[:<class>:]]` - Matches any character belonging to `<class>`.
   *     - `[[:alnum:]]` - Matches any digit or letter.
   *     - `[[:digit:]abc]` - Matches any digit, `a`, `b` or `c`.
   *     - See https://facelessuser.github.io/wcmatch/glob/#posix-character-classes
   *       for a complete list of supported character classes.
   * - `\` - Escapes the next character for an `os` other than `"windows"`.
   * - \` - Escapes the next character for `os` set to `"windows"`.
   * - `/` - Path separator.
   * - `\` - Additional path separator only for `os` set to `"windows"`.
   *
   * Extended syntax:
   * - Requires `{ extended: true }`.
   * - `?(foo|bar)` - Matches 0 or 1 instance of `{foo,bar}`.
   * - `@(foo|bar)` - Matches 1 instance of `{foo,bar}`. They behave the same.
   * - `*(foo|bar)` - Matches _n_ instances of `{foo,bar}`.
   * - `+(foo|bar)` - Matches _n > 0_ instances of `{foo,bar}`.
   * - `!(foo|bar)` - Matches anything other than `{foo,bar}`.
   * - See https://www.linuxjournal.com/content/bash-extended-globbing.
   *
   * Globstar syntax:
   * - Requires `{ globstar: true }`.
   * - `**` - Matches any number of any path segments.
   *     - Must comprise its entire path segment in the provided glob.
   * - See https://www.linuxjournal.com/content/globstar-new-bash-globbing-option.
   *
   * Note the following properties:
   * - The generated `RegExp` is anchored at both start and end.
   * - Repeating and trailing separators are tolerated. Trailing separators in the
   *   provided glob have no meaning and are discarded.
   * - Absolute globs will only match absolute paths, etc.
   * - Empty globs will match nothing.
   * - Any special glob syntax must be contained to one path segment. For example,
   *   `?(foo|bar/baz)` is invalid. The separator will take precedence and the
   *   first segment ends with an unclosed group.
   * - If a path segment ends with unclosed groups or a dangling escape prefix, a
   *   parse error has occurred. Every character for that segment is taken
   *   literally in this event.
   *
   * Limitations:
   * - A negative group like `!(foo|bar)` will wrongly be converted to a negative
   *   look-ahead followed by a wildcard. This means that `!(foo).js` will wrongly
   *   fail to match `foobar.js`, even though `foobar` is not `foo`. Effectively,
   *   `!(foo|bar)` is treated like `!(@(foo|bar)*)`. This will work correctly if
   *   the group occurs not nested at the end of the segment.
   */
  globToRegExp,
} from "./glob_to_regexp.ts";

export {
  /**
   * @deprecated (will be removed in 0.215.0) Import from {@link https://deno.land/std/path/posix/normalize_glob.ts} instead.
   *
   * Like normalize(), but doesn't collapse "**\/.." when `globstar` is true.
   */
  normalizeGlob,
} from "./normalize_glob.ts";

export {
  /**
   * @deprecated (will be removed in 0.215.0) Import from {@link https://deno.land/std/path/posix/join_globs.ts} instead.
   *
   * Like join(), but doesn't collapse "**\/.." when `globstar` is true.
   */
  joinGlobs,
} from "./join_globs.ts";
