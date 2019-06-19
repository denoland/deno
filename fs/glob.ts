import { globrex } from "./globrex.ts";

export interface GlobOptions {
  // Allow ExtGlob features
  extended?: boolean;
  // When globstar is true, '/foo/**' is equivelant
  // to '/foo/*' when globstar is false.
  // Having globstar set to true is the same usage as
  // using wildcards in bash
  globstar?: boolean;
  // be laissez faire about mutiple slashes
  strict?: boolean;
  // Parse as filepath for extra path related features
  filepath?: boolean;
  // Flag to use in the generated RegExp
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
 *       match: [glob("*.ts")]
 *     })
 *
 *     Looking for all the `.json` files in any subfolder:
 *     walkSync(".", {
 *       match: [glob(join("a", "**", "*.json"),{
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
export function glob(glob: string, options: GlobOptions = {}): RegExp {
  return globrex(glob, options).regex;
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
