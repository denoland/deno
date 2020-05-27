// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

interface Code {
  open: string;
  close: string;
  regexp: RegExp;
}

function code(open: number, close: number): Code {
  return {
    open: `\x1b[${open}m`,
    close: `\x1b[${close}m`,
    regexp: new RegExp(`\\x1b\\[${close}m`, "g"),
  };
}

function run(str: string, code: Code): string {
  return !globalThis || !globalThis.Deno || globalThis.Deno.noColor
    ? str
    : `${code.open}${str.replace(code.regexp, code.open)}${code.close}`;
}

export function bold(str: string): string {
  return run(str, code(1, 22));
}

export function italic(str: string): string {
  return run(str, code(3, 23));
}

export function yellow(str: string): string {
  return run(str, code(33, 39));
}

export function cyan(str: string): string {
  return run(str, code(36, 39));
}

export function red(str: string): string {
  return run(str, code(31, 39));
}

export function green(str: string): string {
  return run(str, code(32, 39));
}

export function bgRed(str: string): string {
  return run(str, code(41, 49));
}

export function white(str: string): string {
  return run(str, code(37, 39));
}

export function gray(str: string): string {
  return run(str, code(90, 39));
}

export function magenta(str: string): string {
  return run(str, code(35, 39));
}

export function dim(str: string): string {
  return run(str, code(2, 22));
}

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
