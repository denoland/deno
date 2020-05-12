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

export function reset(str: string): string {
  return run(str, code([0], 0));
}
export function bold(str: string): string {
  return run(str, code([1], 22));
}
export function dim(str: string): string {
  return run(str, code([2], 22));
}
export function italic(str: string): string {
  return run(str, code([3], 23));
}
export function underline(str: string): string {
  return run(str, code([4], 24));
}
export function inverse(str: string): string {
  return run(str, code([7], 27));
}
export function hidden(str: string): string {
  return run(str, code([8], 28));
}
export function strikethrough(str: string): string {
  return run(str, code([9], 29));
}

export function black(str: string): string {
  return run(str, code([30], 39));
}
export function red(str: string): string {
  return run(str, code([31], 39));
}
export function green(str: string): string {
  return run(str, code([32], 39));
}
export function yellow(str: string): string {
  return run(str, code([33], 39));
}
export function blue(str: string): string {
  return run(str, code([34], 39));
}
export function magenta(str: string): string {
  return run(str, code([35], 39));
}
export function cyan(str: string): string {
  return run(str, code([36], 39));
}
export function white(str: string): string {
  return run(str, code([37], 39));
}
export function gray(str: string): string {
  return run(str, code([90], 39));
}

export function bgBlack(str: string): string {
  return run(str, code([40], 49));
}
export function bgRed(str: string): string {
  return run(str, code([41], 49));
}
export function bgGreen(str: string): string {
  return run(str, code([42], 49));
}
export function bgYellow(str: string): string {
  return run(str, code([43], 49));
}
export function bgBlue(str: string): string {
  return run(str, code([44], 49));
}
export function bgMagenta(str: string): string {
  return run(str, code([45], 49));
}
export function bgCyan(str: string): string {
  return run(str, code([46], 49));
}
export function bgWhite(str: string): string {
  return run(str, code([47], 49));
}

// https://github.com/chalk/ansi-regex/blob/2b56fb0c7a07108e5b54241e8faec160d393aedb/index.js
// Speykious: hard-coded the regex, because no need to use a joined list
const ANSI_PATTERN = /[\u001B\u009B][[\]()#;?]*(?:(?:(?:[a-zA-Z\d]*(?:;[-a-zA-Z\d\/#&.:=?%@~_]*)*)?\u0007)|(?:(?:\\d{1,4}(?:;\\d{0,4})*)?[\\dA-PR-TZcf-ntqry=><~]))/g;

/** Removes all ANSI escape sequences from a string. */
export function stripColor(string: string): string {
  return string.replace(ANSI_PATTERN, "");
}

/** Converts a hex color to an [r,g,b] array. */
function hexToRgbArray(color: number): [number, number, number] {
  color = color & 0xffffff;

  return [
    (color >> 16) & 0xff, // red
    (color >> 8) & 0xff, // green
    color & 0xff, // blue
  ];
}

/**
 * Set text color using 24bit rgb.
 * @param color A hexadecimal number representing an RGB color, like 0xff3232 or 0x98cc02
 */
export function rgb24(color: number): Code {
  return code([38, 2, ...hexToRgbArray(color)], 39);
}

/**
 * Set text background color using 24bit rgb.
 * @param color A hexadecimal number representing an RGB color, like 0xff3232 or 0x98cc02
 */
export function bgRgb24(color: number): Code {
  return code([48, 2, ...hexToRgbArray(color)], 49);
}
