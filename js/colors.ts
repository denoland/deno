// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// TODO(kitsonk) Replace with `deno_std/colors/mod.ts` when we can load modules
// which end in `.ts`.

import { noColor } from "./deno.ts";

interface Code {
  open: string;
  close: string;
  regexp: RegExp;
}

let enabled = !noColor;

function code(open: number, close: number): Code {
  return {
    open: `\x1b[${open}m`,
    close: `\x1b[${close}m`,
    regexp: new RegExp(`\\x1b\\[${close}m`, "g")
  };
}

function run(str: string, code: Code): string {
  return enabled
    ? `${code.open}${str.replace(code.regexp, code.open)}${code.close}`
    : str;
}

export function bold(str: string): string {
  return run(str, code(1, 22));
}

export function yellow(str: string): string {
  return run(str, code(33, 39));
}

export function cyan(str: string): string {
  return run(str, code(36, 39));
}
