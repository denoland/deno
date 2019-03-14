// This file is ported from globrex@0.1.2
// MIT License
// Copyright (c) 2018 Terkel Gjervig Nielsen

import { GlobOptions } from "./glob.ts";

const isWin = Deno.build.os === "win";
const SEP = isWin ? `\\\\+` : `\\/`;
const SEP_ESC = isWin ? `\\\\` : `/`;
const GLOBSTAR = `((?:[^/]*(?:/|$))*)`;
const WILDCARD = `([^/]*)`;
const GLOBSTAR_SEGMENT = `((?:[^${SEP_ESC}]*(?:${SEP_ESC}|$))*)`;
const WILDCARD_SEGMENT = `([^${SEP_ESC}]*)`;

export interface GlobrexResult {
  regex: RegExp;
  path?: {
    regex: string | RegExp;
    segments: RegExp[];
    globstar?: RegExp;
  };
}

/**
 * Convert any glob pattern to a JavaScript Regexp object
 * @param glob Glob pattern to convert
 * @param opts Configuration object
 * @param [opts.extended=false] Support advanced ext globbing
 * @param [opts.globstar=false] Support globstar
 * @param [opts.strict=true] be laissez faire about mutiple slashes
 * @param [opts.filepath=""] Parse as filepath for extra path related features
 * @param [opts.flags=""] RegExp globs
 * @returns Converted object with string, segments and RegExp object
 */
export function globrex(
  glob: string,
  {
    extended = false,
    globstar = false,
    strict = false,
    filepath = false,
    flags = ""
  }: GlobOptions = {}
): GlobrexResult {
  let regex = "";
  let segment = "";
  let path: {
    regex: string | RegExp;
    segments: RegExp[];
    globstar?: RegExp;
  } = { regex: "", segments: [] };

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
    str,
    options: AddOptions = { split: false, last: false, only: "" }
  ): void {
    const { split, last, only } = options;
    if (only !== "path") regex += str;
    if (filepath && only !== "regex") {
      path.regex += str === "\\/" ? SEP : str;
      if (split) {
        if (last) segment += str;
        if (segment !== "") {
          if (!flags.includes("g")) segment = `^${segment}$`; // change it 'includes'
          path.segments.push(new RegExp(segment, flags));
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

    if (c === "/") {
      add(`\\${c}`, { split: true });
      if (n === "/" && !strict) regex += "?";
      continue;
    }

    if (c === "(") {
      if (ext.length) {
        add(c);
        continue;
      }
      add(`\\${c}`);
      continue;
    }

    if (c === ")") {
      if (ext.length) {
        add(c);
        let type = ext.pop();
        if (type === "@") {
          add("{1}");
        } else if (type === "!") {
          add("([^/]*)");
        } else {
          add(type);
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
        if (value === "alnum") add("(\\w|\\d)");
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
        add("(");
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
      let prevChar = glob[i - 1];
      let starCount = 1;
      while (glob[i + 1] === "*") {
        starCount++;
        i++;
      }
      let nextChar = glob[i + 1];
      if (!globstar) {
        // globstar is disabled, so treat any number of "*" as one
        add(".*");
      } else {
        // globstar is enabled, so determine if this is a globstar segment
        let isGlobstar =
          starCount > 1 && // multiple "*"'s
          (prevChar === "/" || prevChar === undefined) && // from the start of the segment
          (nextChar === "/" || nextChar === undefined); // to the end of the segment
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
    if (filepath) path.regex = `^${path.regex}$`;
  }

  const result: GlobrexResult = { regex: new RegExp(regex, flags) };

  // Push the last segment
  if (filepath) {
    path.segments.push(new RegExp(segment, flags));
    path.regex = new RegExp(path.regex.toString(), flags);
    path.globstar = new RegExp(
      !flags.includes("g") ? `^${GLOBSTAR_SEGMENT}$` : GLOBSTAR_SEGMENT,
      flags
    );
    result.path = path;
  }

  return result;
}
