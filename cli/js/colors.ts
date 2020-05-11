// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// TODO(kitsonk) Replace with `deno_std/colors/mod.ts` when we can load modules
// which end in `.ts`.

import { noColor } from "./deno.ts";

interface Code {
  open: string;
  close: string;
  regexp: RegExp;
}

const enabled = !noColor;

function code(open: number[], close: number): Code {
  return {
    open: `\x1b[${open.join(";")}m`,
    close: `\x1b[${close}m`,
    regexp: new RegExp(`\\x1b\\[${close}m`, "g"),
  };
}

function run(str: string, code: Code): string {
  return enabled
    ? `${code.open}${str.replace(code.regexp, code.open)}${code.close}`
    : str;
}

export const reset         = (str: string) => run(str, code([0],  0))
export const bold          = (str: string) => run(str, code([1],  22))
export const dim           = (str: string) => run(str, code([2],  22))
export const italic        = (str: string) => run(str, code([3],  23))
export const underline     = (str: string) => run(str, code([4],  24))
export const inverse       = (str: string) => run(str, code([7],  27))
export const hidden        = (str: string) => run(str, code([8],  28))
export const strikethrough = (str: string) => run(str, code([9],  29))

export const black         = (str: string) => run(str, code([30], 39))
export const red           = (str: string) => run(str, code([31], 39))
export const green         = (str: string) => run(str, code([32], 39))
export const yellow        = (str: string) => run(str, code([33], 39))
export const blue          = (str: string) => run(str, code([34], 39))
export const magenta       = (str: string) => run(str, code([35], 39))
export const cyan          = (str: string) => run(str, code([36], 39))
export const white         = (str: string) => run(str, code([37], 39))
export const gray          = (str: string) => run(str, code([90], 39))

export const bgBlack       = (str: string) => run(str, code([40], 49))
export const bgRed         = (str: string) => run(str, code([41], 49))
export const bgGreen       = (str: string) => run(str, code([42], 49))
export const bgYellow      = (str: string) => run(str, code([43], 49))
export const bgBlue        = (str: string) => run(str, code([44], 49))
export const bgMagenta     = (str: string) => run(str, code([45], 49))
export const bgCyan        = (str: string) => run(str, code([46], 49))
export const bgWhite       = (str: string) => run(str, code([47], 49))

// https://github.com/chalk/ansi-regex/blob/2b56fb0c7a07108e5b54241e8faec160d393aedb/index.js
const ANSI_PATTERN = new RegExp(
  [
    "[\\u001B\\u009B][[\\]()#;?]*(?:(?:(?:[a-zA-Z\\d]*(?:;[-a-zA-Z\\d\\/#&.:=?%@~_]*)*)?\\u0007)",
    "(?:(?:\\d{1,4}(?:;\\d{0,4})*)?[\\dA-PR-TZcf-ntqry=><~]))",
  ].join("|"),
  "g"
);

export function stripColor(string: string): string {
  return string.replace(ANSI_PATTERN, "");
}

