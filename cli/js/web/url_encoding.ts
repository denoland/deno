// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

function charIsC0ControlSet(c: string): boolean {
  return c >= "\u0000" && c <= "\u001F";
}

function charIsSearchSet(c: string): boolean {
  // prettier-ignore
  return charIsC0ControlSet(c) || ["\u0020", "\u0022", "\u0023", "\u0027", "\u003C", "\u003E"].includes(c) || c > "\u007E";
}

function charIsFragmentSet(c: string): boolean {
  // prettier-ignore
  return charIsC0ControlSet(c) || ["\u0020", "\u0022", "\u003C", "\u003E", "\u0060"].includes(c);
}

function charIsPathSet(c: string): boolean {
  // prettier-ignore
  return charIsFragmentSet(c) || ["\u0023", "\u003F", "\u007B", "\u007D"].includes(c);
}

function charIsUserinfoSet(c: string): boolean {
  // "\u0027" ("'") seemingly isn't in the spec, but matches Chrome and Firefox.
  // prettier-ignore
  return charIsPathSet(c) || ["\u0027", "\u002F", "\u003A", "\u003B", "\u003D", "\u0040", "\u005B", "\u005C", "\u005D", "\u005E", "\u007C"].includes(c);
}

function encodeChar(c: string): string {
  return `%${c.charCodeAt(0).toString(16)}`.toUpperCase();
}

export function encodeUserinfo(s: string): string {
  return [...s].map((c) => (charIsUserinfoSet(c) ? encodeChar(c) : c)).join("");
}

export function encodeHostname(s: string): string {
  // FIXME(nayeemrmn)
  return encodeURIComponent(s);
}

export function encodePathname(s: string): string {
  return [...s].map((c) => (charIsPathSet(c) ? encodeChar(c) : c)).join("");
}

export function encodeSearch(s: string): string {
  return [...s].map((c) => (charIsSearchSet(c) ? encodeChar(c) : c)).join("");
}

export function encodeHash(s: string): string {
  return [...s].map((c) => (charIsFragmentSet(c) ? encodeChar(c) : c)).join("");
}
