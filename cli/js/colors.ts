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

function code(open: number, close: number): Code {
  return {
    open: `\x1b[${open}m`,
    close: `\x1b[${close}m`,
    regexp: new RegExp(`\\x1b\\[${close}m`, "g"),
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
