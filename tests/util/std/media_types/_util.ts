// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/** Supporting functions for media_types that do not make part of the public
 * API.
 *
 * @module
 * @private
 */
export interface DBEntry {
  source: string;
  compressible?: boolean;
  charset?: string;
  extensions?: string[];
}

/** A map of extensions for a given media type. */
export const extensions = new Map<string, string[]>();

export function consumeToken(v: string): [token: string, rest: string] {
  const notPos = indexOf(v, isNotTokenChar);
  if (notPos === -1) {
    return [v, ""];
  }
  if (notPos === 0) {
    return ["", v];
  }
  return [v.slice(0, notPos), v.slice(notPos)];
}

export function consumeValue(v: string): [value: string, rest: string] {
  if (!v) {
    return ["", v];
  }
  if (v[0] !== `"`) {
    return consumeToken(v);
  }
  let value = "";
  for (let i = 1; i < v.length; i++) {
    const r = v[i];
    if (r === `"`) {
      return [value, v.slice(i + 1)];
    }
    if (r === "\\" && i + 1 < v.length && isTSpecial(v[i + 1])) {
      value += v[i + 1];
      i++;
      continue;
    }
    if (r === "\r" || r === "\n") {
      return ["", v];
    }
    value += v[i];
  }
  return ["", v];
}

export function consumeMediaParam(
  v: string,
): [key: string, value: string, rest: string] {
  let rest = v.trimStart();
  if (!rest.startsWith(";")) {
    return ["", "", v];
  }
  rest = rest.slice(1);
  rest = rest.trimStart();
  let param: string;
  [param, rest] = consumeToken(rest);
  param = param.toLowerCase();
  if (!param) {
    return ["", "", v];
  }
  rest = rest.slice(1);
  rest = rest.trimStart();
  const [value, rest2] = consumeValue(rest);
  if (value === "" && rest2 === rest) {
    return ["", "", v];
  }
  rest = rest2;
  return [param, value, rest];
}

export function decode2331Encoding(v: string): string | undefined {
  const sv = v.split(`'`, 3);
  if (sv.length !== 3) {
    return undefined;
  }
  const charset = sv[0].toLowerCase();
  if (!charset) {
    return undefined;
  }
  if (charset !== "us-ascii" && charset !== "utf-8") {
    return undefined;
  }
  const encv = decodeURI(sv[2]);
  if (!encv) {
    return undefined;
  }
  return encv;
}

function indexOf<T>(s: Iterable<T>, fn: (s: T) => boolean): number {
  let i = -1;
  for (const v of s) {
    i++;
    if (fn(v)) {
      return i;
    }
  }
  return -1;
}

export function isIterator<T>(obj: unknown): obj is Iterable<T> {
  if (obj === null || obj === undefined) {
    return false;
  }
  // deno-lint-ignore no-explicit-any
  return typeof (obj as any)[Symbol.iterator] === "function";
}

export function isToken(s: string): boolean {
  if (!s) {
    return false;
  }
  return indexOf(s, isNotTokenChar) < 0;
}

function isNotTokenChar(r: string): boolean {
  return !isTokenChar(r);
}

function isTokenChar(r: string): boolean {
  const code = r.charCodeAt(0);
  return code > 0x20 && code < 0x7f && !isTSpecial(r);
}

function isTSpecial(r: string): boolean {
  return `()<>@,;:\\"/[]?=`.includes(r[0]);
}

const CHAR_CODE_SPACE = " ".charCodeAt(0);
const CHAR_CODE_TILDE = "~".charCodeAt(0);

export function needsEncoding(s: string): boolean {
  for (const b of s) {
    const charCode = b.charCodeAt(0);
    if (
      (charCode < CHAR_CODE_SPACE || charCode > CHAR_CODE_TILDE) && b !== "\t"
    ) {
      return true;
    }
  }
  return false;
}
