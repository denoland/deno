// globToRegExp() is originall ported from globrex@0.1.2.
// Copyright 2018 Terkel Gjervig Nielsen. All rights reserved. MIT license.
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { NATIVE_OS } from "./_constants.ts";
import { join, normalize } from "./mod.ts";
import { SEP, SEP_PATTERN } from "./separator.ts";

export interface GlobOptions {
  /** Extended glob syntax.
   * See https://www.linuxjournal.com/content/bash-extended-globbing. Defaults
   * to true. */
  extended?: boolean;
  /** Globstar syntax.
   * See https://www.linuxjournal.com/content/globstar-new-bash-globbing-option.
   * If false, `**` is treated like `*`. Defaults to true. */
  globstar?: boolean;
  /** Operating system. Defaults to the native OS. */
  os?: typeof Deno.build.os;
}

export type GlobToRegExpOptions = GlobOptions;

/** Convert a glob string to a regular expressions.
 *
 *      // Looking for all the `ts` files:
 *      walkSync(".", {
 *        match: [globToRegExp("*.ts")]
 *      });
 *
 *      Looking for all the `.json` files in any subfolder:
 *      walkSync(".", {
 *        match: [globToRegExp(join("a", "**", "*.json"), {
 *          extended: true,
 *          globstar: true
 *        })]
 *      }); */
export function globToRegExp(
  glob: string,
  { extended = true, globstar: globstarOption = true, os = NATIVE_OS }:
    GlobToRegExpOptions = {},
): RegExp {
  const sep = os == "windows" ? `(?:\\\\|\\/)+` : `\\/+`;
  const sepMaybe = os == "windows" ? `(?:\\\\|\\/)*` : `\\/*`;
  const seps = os == "windows" ? ["\\", "/"] : ["/"];
  const sepRaw = os == "windows" ? `\\` : `/`;
  const globstar = os == "windows"
    ? `(?:[^\\\\/]*(?:\\\\|\\/|$)+)*`
    : `(?:[^/]*(?:\\/|$)+)*`;
  const wildcard = os == "windows" ? `[^\\\\/]*` : `[^/]*`;

  // Keep track of scope for extended syntaxes.
  const extStack = [];

  // If we are doing extended matching, this boolean is true when we are inside
  // a group (eg {*.html,*.js}), and false otherwise.
  let inGroup = false;
  let inRange = false;

  let regExpString = "";

  // Remove trailing separators.
  let newLength = glob.length;
  for (; newLength > 0 && seps.includes(glob[newLength - 1]); newLength--);
  glob = glob.slice(0, newLength);

  let c, n;
  for (let i = 0; i < glob.length; i++) {
    c = glob[i];
    n = glob[i + 1];

    if (seps.includes(c)) {
      regExpString += sep;
      while (seps.includes(glob[i + 1])) i++;
      continue;
    }

    if (c == "[") {
      if (inRange && n == ":") {
        i++; // skip [
        let value = "";
        while (glob[++i] !== ":") value += glob[i];
        if (value == "alnum") regExpString += "\\w\\d";
        else if (value == "space") regExpString += "\\s";
        else if (value == "digit") regExpString += "\\d";
        i++; // skip last ]
        continue;
      }
      inRange = true;
      regExpString += c;
      continue;
    }

    if (c == "]") {
      inRange = false;
      regExpString += c;
      continue;
    }

    if (c == "!") {
      if (inRange) {
        if (glob[i - 1] == "[") {
          regExpString += "^";
          continue;
        }
      } else if (extended) {
        if (n == "(") {
          extStack.push(c);
          regExpString += "(?!";
          i++;
          continue;
        }
        regExpString += `\\${c}`;
        continue;
      } else {
        regExpString += `\\${c}`;
        continue;
      }
    }

    if (inRange) {
      if (c == "\\" || c == "^" && glob[i - 1] == "[") regExpString += `\\${c}`;
      else regExpString += c;
      continue;
    }

    if (["\\", "$", "^", ".", "="].includes(c)) {
      regExpString += `\\${c}`;
      continue;
    }

    if (c == "(") {
      if (extStack.length) {
        regExpString += `${c}?:`;
        continue;
      }
      regExpString += `\\${c}`;
      continue;
    }

    if (c == ")") {
      if (extStack.length) {
        regExpString += c;
        const type = extStack.pop()!;
        if (type == "@") {
          regExpString += "{1}";
        } else if (type == "!") {
          regExpString += wildcard;
        } else {
          regExpString += type;
        }
        continue;
      }
      regExpString += `\\${c}`;
      continue;
    }

    if (c == "|") {
      if (extStack.length) {
        regExpString += c;
        continue;
      }
      regExpString += `\\${c}`;
      continue;
    }

    if (c == "+") {
      if (n == "(" && extended) {
        extStack.push(c);
        continue;
      }
      regExpString += `\\${c}`;
      continue;
    }

    if (c == "@" && extended) {
      if (n == "(") {
        extStack.push(c);
        continue;
      }
    }

    if (c == "?") {
      if (extended) {
        if (n == "(") {
          extStack.push(c);
        }
        continue;
      } else {
        regExpString += ".";
        continue;
      }
    }

    if (c == "{") {
      inGroup = true;
      regExpString += "(?:";
      continue;
    }

    if (c == "}") {
      inGroup = false;
      regExpString += ")";
      continue;
    }

    if (c == ",") {
      if (inGroup) {
        regExpString += "|";
        continue;
      }
      regExpString += `\\${c}`;
      continue;
    }

    if (c == "*") {
      if (n == "(" && extended) {
        extStack.push(c);
        continue;
      }
      // Move over all consecutive "*"'s.
      // Also store the previous and next characters
      const prevChar = glob[i - 1];
      let starCount = 1;
      while (glob[i + 1] == "*") {
        starCount++;
        i++;
      }
      const nextChar = glob[i + 1];
      const isGlobstar = globstarOption && starCount > 1 &&
        // from the start of the segment
        [sepRaw, "/", undefined].includes(prevChar) &&
        // to the end of the segment
        [sepRaw, "/", undefined].includes(nextChar);
      if (isGlobstar) {
        // it's a globstar, so match zero or more path segments
        regExpString += globstar;
        while (seps.includes(glob[i + 1])) i++;
      } else {
        // it's not a globstar, so only match one path segment
        regExpString += wildcard;
      }
      continue;
    }

    regExpString += c;
  }

  regExpString = `^${regExpString}${regExpString != "" ? sepMaybe : ""}$`;
  return new RegExp(regExpString);
}

/** Test whether the given string is a glob */
export function isGlob(str: string): boolean {
  const chars: Record<string, string> = { "{": "}", "(": ")", "[": "]" };
  /* eslint-disable-next-line max-len */
  const regex =
    /\\(.)|(^!|\*|[\].+)]\?|\[[^\\\]]+\]|\{[^\\}]+\}|\(\?[:!=][^\\)]+\)|\([^|]+\|[^\\)]+\))/;

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
  { globstar = false }: GlobOptions = {},
): string {
  if (glob.match(/\0/g)) {
    throw new Error(`Glob contains invalid characters: "${glob}"`);
  }
  if (!globstar) {
    return normalize(glob);
  }
  const s = SEP_PATTERN.source;
  const badParentPattern = new RegExp(
    `(?<=(${s}|^)\\*\\*${s})\\.\\.(?=${s}|$)`,
    "g",
  );
  return normalize(glob.replace(badParentPattern, "\0")).replace(/\0/g, "..");
}

/** Like join(), but doesn't collapse "**\/.." when `globstar` is true. */
export function joinGlobs(
  globs: string[],
  { extended = false, globstar = false }: GlobOptions = {},
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
