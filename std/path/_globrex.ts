// This file is ported from globrex@0.1.2
// MIT License
// Copyright (c) 2018 Terkel Gjervig Nielsen
/** This module is browser compatible. */

import { isWindows as isWin } from "./_constants.ts";

const SEP = isWin ? `(?:\\\\|\\/)` : `\\/`;
const SEP_ESC = isWin ? `\\\\` : `/`;
const SEP_RAW = isWin ? `\\` : `/`;
const GLOBSTAR = `(?:(?:[^${SEP_ESC}/]*(?:${SEP_ESC}|\/|$))*)`;
const WILDCARD = `(?:[^${SEP_ESC}/]*)`;
const GLOBSTAR_SEGMENT = `((?:[^${SEP_ESC}/]*(?:${SEP_ESC}|\/|$))*)`;
const WILDCARD_SEGMENT = `(?:[^${SEP_ESC}/]*)`;

export interface GlobrexOptions {
  /** Allow ExtGlob features.
   * @default false */
  extended?: boolean;
  /** Support globstar.
   * @remarks When globstar is `true`, '/foo/**' is equivalent
   * to '/foo/*' when globstar is `false`.
   * Having globstar set to `true` is the same usage as
   * using wildcards in bash.
   * @default false */
  globstar?: boolean;
  /** Be laissez-faire about mutiple slashes.
   * @default true */
  strict?: boolean;
  /** Parse as filepath for extra path related features.
   * @default false */
  filepath?: boolean;
  /** Flag to use in the generated RegExp. */
  flags?: string;
}

export interface GlobrexResult {
  regex: RegExp;
  path?: {
    regex: RegExp;
    segments: RegExp[];
    globstar?: RegExp;
  };
}

/**
 * Convert any glob pattern to a JavaScript Regexp object
 * @param glob Glob pattern to convert
 * @param opts Configuration object
 * @returns Converted object with string, segments and RegExp object
 */
export function globrex(
  glob: string,
  {
    extended = false,
    globstar = false,
    strict = false,
    filepath = false,
    flags = "",
  }: GlobrexOptions = {}
): GlobrexResult {
  const sepPattern = new RegExp(`^${SEP}${strict ? "" : "+"}$`);
  let regex = "";
  let segment = "";
  let pathRegexStr = "";
  const pathSegments = [];

  // If we are doing extended matching, this boolean is true when we are inside
  // a group (eg {*.html,*.js}), and false otherwise.
  let inGroup = false;
  let inRange = false;

  // extglob stack. Keep track of scope
  const ext = [];

  interface AddOptions {
    split?: boolean;
    last?: boolean;
    only?: string;
  }

  // Helper function to build string and segments
  function add(
    str: string,
    options: AddOptions = { split: false, last: false, only: "" }
  ): void {
    const { split, last, only } = options;
    if (only !== "path") regex += str;
    if (filepath && only !== "regex") {
      pathRegexStr += str.match(sepPattern) ? SEP : str;
      if (split) {
        if (last) segment += str;
        if (segment !== "") {
          // change it 'includes'
          if (!flags.includes("g")) segment = `^${segment}$`;
          pathSegments.push(new RegExp(segment, flags));
        }
        segment = "";
      } else {
        segment += str;
      }
    }
  }

  let c, n;
  for (let i = 0; i < glob.length; i++) {
    c = glob[i];
    n = glob[i + 1];

    if (["\\", "$", "^", ".", "="].includes(c)) {
      add(`\\${c}`);
      continue;
    }

    if (c.match(sepPattern)) {
      add(SEP, { split: true });
      if (n != null && n.match(sepPattern) && !strict) regex += "?";
      continue;
    }

    if (c === "(") {
      if (ext.length) {
        add(`${c}?:`);
        continue;
      }
      add(`\\${c}`);
      continue;
    }

    if (c === ")") {
      if (ext.length) {
        add(c);
        const type: string | undefined = ext.pop();
        if (type === "@") {
          add("{1}");
        } else if (type === "!") {
          add(WILDCARD);
        } else {
          add(type as string);
        }
        continue;
      }
      add(`\\${c}`);
      continue;
    }

    if (c === "|") {
      if (ext.length) {
        add(c);
        continue;
      }
      add(`\\${c}`);
      continue;
    }

    if (c === "+") {
      if (n === "(" && extended) {
        ext.push(c);
        continue;
      }
      add(`\\${c}`);
      continue;
    }

    if (c === "@" && extended) {
      if (n === "(") {
        ext.push(c);
        continue;
      }
    }

    if (c === "!") {
      if (extended) {
        if (inRange) {
          add("^");
          continue;
        }
        if (n === "(") {
          ext.push(c);
          add("(?!");
          i++;
          continue;
        }
        add(`\\${c}`);
        continue;
      }
      add(`\\${c}`);
      continue;
    }

    if (c === "?") {
      if (extended) {
        if (n === "(") {
          ext.push(c);
        } else {
          add(".");
        }
        continue;
      }
      add(`\\${c}`);
      continue;
    }

    if (c === "[") {
      if (inRange && n === ":") {
        i++; // skip [
        let value = "";
        while (glob[++i] !== ":") value += glob[i];
        if (value === "alnum") add("(?:\\w|\\d)");
        else if (value === "space") add("\\s");
        else if (value === "digit") add("\\d");
        i++; // skip last ]
        continue;
      }
      if (extended) {
        inRange = true;
        add(c);
        continue;
      }
      add(`\\${c}`);
      continue;
    }

    if (c === "]") {
      if (extended) {
        inRange = false;
        add(c);
        continue;
      }
      add(`\\${c}`);
      continue;
    }

    if (c === "{") {
      if (extended) {
        inGroup = true;
        add("(?:");
        continue;
      }
      add(`\\${c}`);
      continue;
    }

    if (c === "}") {
      if (extended) {
        inGroup = false;
        add(")");
        continue;
      }
      add(`\\${c}`);
      continue;
    }

    if (c === ",") {
      if (inGroup) {
        add("|");
        continue;
      }
      add(`\\${c}`);
      continue;
    }

    if (c === "*") {
      if (n === "(" && extended) {
        ext.push(c);
        continue;
      }
      // Move over all consecutive "*"'s.
      // Also store the previous and next characters
      const prevChar = glob[i - 1];
      let starCount = 1;
      while (glob[i + 1] === "*") {
        starCount++;
        i++;
      }
      const nextChar = glob[i + 1];
      if (!globstar) {
        // globstar is disabled, so treat any number of "*" as one
        add(".*");
      } else {
        // globstar is enabled, so determine if this is a globstar segment
        const isGlobstar =
          starCount > 1 && // multiple "*"'s
          // from the start of the segment
          [SEP_RAW, "/", undefined].includes(prevChar) &&
          // to the end of the segment
          [SEP_RAW, "/", undefined].includes(nextChar);
        if (isGlobstar) {
          // it's a globstar, so match zero or more path segments
          add(GLOBSTAR, { only: "regex" });
          add(GLOBSTAR_SEGMENT, { only: "path", last: true, split: true });
          i++; // move over the "/"
        } else {
          // it's not a globstar, so only match one path segment
          add(WILDCARD, { only: "regex" });
          add(WILDCARD_SEGMENT, { only: "path" });
        }
      }
      continue;
    }

    add(c);
  }

  // When regexp 'g' flag is specified don't
  // constrain the regular expression with ^ & $
  if (!flags.includes("g")) {
    regex = `^${regex}$`;
    segment = `^${segment}$`;
    if (filepath) pathRegexStr = `^${pathRegexStr}$`;
  }

  const result: GlobrexResult = { regex: new RegExp(regex, flags) };

  // Push the last segment
  if (filepath) {
    pathSegments.push(new RegExp(segment, flags));
    result.path = {
      regex: new RegExp(pathRegexStr, flags),
      segments: pathSegments,
      globstar: new RegExp(
        !flags.includes("g") ? `^${GLOBSTAR_SEGMENT}$` : GLOBSTAR_SEGMENT,
        flags
      ),
    };
  }

  return result;
}
