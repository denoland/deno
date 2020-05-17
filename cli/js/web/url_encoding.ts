// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

function charInC0ControlSet(c: string): boolean {
  return c >= "\u0000" && c <= "\u001F";
}

function charInSearchSet(c: string): boolean {
  // prettier-ignore
  return charInC0ControlSet(c) || ["\u0020", "\u0022", "\u0023", "\u0027", "\u003C", "\u003E"].includes(c) || c > "\u007E";
}

function charInFragmentSet(c: string): boolean {
  // prettier-ignore
  return charInC0ControlSet(c) || ["\u0020", "\u0022", "\u003C", "\u003E", "\u0060"].includes(c);
}

function charInPathSet(c: string): boolean {
  // prettier-ignore
  return charInFragmentSet(c) || ["\u0023", "\u003F", "\u007B", "\u007D"].includes(c);
}

function charInUserinfoSet(c: string): boolean {
  // "\u0027" ("'") seemingly isn't in the spec, but matches Chrome and Firefox.
  // prettier-ignore
  return charInPathSet(c) || ["\u0027", "\u002F", "\u003A", "\u003B", "\u003D", "\u0040", "\u005B", "\u005C", "\u005D", "\u005E", "\u007C"].includes(c);
}

function encodeChar(c: string): string {
  return `%${c.charCodeAt(0).toString(16)}`.toUpperCase();
}

export function encodeUserinfo(s: string): string {
  return [...s].map((c) => (charInUserinfoSet(c) ? encodeChar(c) : c)).join("");
}

export function encodeHostname(s: string): string {
  // FIXME: https://url.spec.whatwg.org/#idna
  if (s.includes(":")) {
    throw new TypeError("Invalid hostname.");
  }
  return encodeURIComponent(s);
}

export function encodePathname(s: string): string {
  return [...s].map((c) => (charInPathSet(c) ? encodeChar(c) : c)).join("");
}

export function encodeSearch(s: string): string {
  return [...s].map((c) => (charInSearchSet(c) ? encodeChar(c) : c)).join("");
}

export function encodeHash(s: string): string {
  return [...s].map((c) => (charInFragmentSet(c) ? encodeChar(c) : c)).join("");
}
