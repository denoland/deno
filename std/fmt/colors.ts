// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
/**
 * A module to print ANSI terminal colors. Inspired by chalk, kleur, and colors
 * on npm.
 *
 * ```
 * import { bgBlue, red, bold } from "https://deno.land/std/fmt/colors.ts";
 * console.log(bgBlue(red(bold("Hello world!"))));
 * ```
 *
 * This module supports `NO_COLOR` environmental variable disabling any coloring
 * if `NO_COLOR` is set.
 */
const { noColor } = Deno;

interface Code {
  open: string;
  close: string;
  regexp: RegExp;
}

let enabled = !noColor;

export function setColorEnabled(value: boolean): void {
  if (noColor) {
    return;
  }

  enabled = value;
}

export function getColorEnabled(): boolean {
  return enabled;
}

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

export function reset(str: string): string {
  return run(str, code(0, 0));
}

export function bold(str: string): string {
  return run(str, code(1, 22));
}

export function dim(str: string): string {
  return run(str, code(2, 22));
}

export function italic(str: string): string {
  return run(str, code(3, 23));
}

export function underline(str: string): string {
  return run(str, code(4, 24));
}

export function inverse(str: string): string {
  return run(str, code(7, 27));
}

export function hidden(str: string): string {
  return run(str, code(8, 28));
}

export function strikethrough(str: string): string {
  return run(str, code(9, 29));
}

export function black(str: string): string {
  return run(str, code(30, 39));
}

export function red(str: string): string {
  return run(str, code(31, 39));
}

export function green(str: string): string {
  return run(str, code(32, 39));
}

export function yellow(str: string): string {
  return run(str, code(33, 39));
}

export function blue(str: string): string {
  return run(str, code(34, 39));
}

export function magenta(str: string): string {
  return run(str, code(35, 39));
}

export function cyan(str: string): string {
  return run(str, code(36, 39));
}

export function white(str: string): string {
  return run(str, code(37, 39));
}

export function gray(str: string): string {
  return run(str, code(90, 39));
}

export function bgBlack(str: string): string {
  return run(str, code(40, 49));
}

export function bgRed(str: string): string {
  return run(str, code(41, 49));
}

export function bgGreen(str: string): string {
  return run(str, code(42, 49));
}

export function bgYellow(str: string): string {
  return run(str, code(43, 49));
}

export function bgBlue(str: string): string {
  return run(str, code(44, 49));
}

export function bgMagenta(str: string): string {
  return run(str, code(45, 49));
}

export function bgCyan(str: string): string {
  return run(str, code(46, 49));
}

export function bgWhite(str: string): string {
  return run(str, code(47, 49));
}
