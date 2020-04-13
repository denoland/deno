// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

interface ParseOptions {
  decodeURIComponent?: (string: string) => string;
  maxKeys?: number;
}

export function parse(
  str: string,
  sep = "&",
  eq = "=",
  { decodeURIComponent = unescape, maxKeys = 1000 }: ParseOptions = {}
): { [key: string]: string[] | string } {
  const entries = str
    .split(sep)
    .map((entry) => entry.split(eq).map(decodeURIComponent));
  const final: { [key: string]: string[] | string } = {};

  let i = 0;
  while (true) {
    if ((Object.keys(final).length === maxKeys && !!maxKeys) || !entries[i]) {
      break;
    }

    const [key, val] = entries[i];
    if (final[key]) {
      if (Array.isArray(final[key])) {
        (final[key] as string[]).push(val);
      } else {
        final[key] = [final[key] as string, val];
      }
    } else {
      final[key] = val;
    }

    i++;
  }

  return final;
}

interface StringifyOptions {
  encodeURIComponent?: (string: string) => string;
}

export function stringify(
  obj: object,
  sep = "&",
  eq = "=",
  { encodeURIComponent = escape }: StringifyOptions = {}
): string {
  const final = [];

  for (const entry of Object.entries(obj)) {
    if (Array.isArray(entry[1])) {
      for (const val of entry[1]) {
        final.push(encodeURIComponent(entry[0]) + eq + encodeURIComponent(val));
      }
    } else if (typeof entry[1] !== "object" && entry[1] !== undefined) {
      final.push(entry.map(encodeURIComponent).join(eq));
    } else {
      final.push(encodeURIComponent(entry[0]) + eq);
    }
  }

  return final.join(sep);
}

export const decode = parse;
export const encode = stringify;
export const unescape = decodeURIComponent;
export const escape = encodeURIComponent;
