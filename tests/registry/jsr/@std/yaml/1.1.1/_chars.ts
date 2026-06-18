// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2026 the Deno authors. MIT license.

export const BOM = 0xfeff; /* BOM */
export const TAB = 0x09; /* Tab */
export const LINE_FEED = 0x0a; /* LF */
export const CARRIAGE_RETURN = 0x0d; /* CR */
export const SPACE = 0x20; /* Space */
export const EXCLAMATION = 0x21; /* ! */
export const DOUBLE_QUOTE = 0x22; /* " */
export const SHARP = 0x23; /* # */
export const PERCENT = 0x25; /* % */
export const AMPERSAND = 0x26; /* & */
export const SINGLE_QUOTE = 0x27; /* ' */
export const ASTERISK = 0x2a; /* * */
export const PLUS = 0x2b; /* + */
export const COMMA = 0x2c; /* , */
export const MINUS = 0x2d; /* - */
export const DOT = 0x2e; /* . */
export const COLON = 0x3a; /* : */
export const SMALLER_THAN = 0x3c; /* < */
export const GREATER_THAN = 0x3e; /* > */
export const QUESTION = 0x3f; /* ? */
export const COMMERCIAL_AT = 0x40; /* @ */
export const LEFT_SQUARE_BRACKET = 0x5b; /* [ */
export const BACKSLASH = 0x5c; /* \ */
export const RIGHT_SQUARE_BRACKET = 0x5d; /* ] */
export const GRAVE_ACCENT = 0x60; /* ` */
export const LEFT_CURLY_BRACKET = 0x7b; /* { */
export const VERTICAL_LINE = 0x7c; /* | */
export const RIGHT_CURLY_BRACKET = 0x7d; /* } */

export function isEOL(c: number): boolean {
  return c === LINE_FEED || c === CARRIAGE_RETURN;
}

export function isWhiteSpace(c: number): boolean {
  return c === TAB || c === SPACE;
}

export function isWhiteSpaceOrEOL(c: number): boolean {
  return isWhiteSpace(c) || isEOL(c);
}

export function isFlowIndicator(c: number): boolean {
  return (
    c === COMMA ||
    c === LEFT_SQUARE_BRACKET ||
    c === RIGHT_SQUARE_BRACKET ||
    c === LEFT_CURLY_BRACKET ||
    c === RIGHT_CURLY_BRACKET
  );
}
