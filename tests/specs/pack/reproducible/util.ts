import ansiRegex from "ansi-regex";

export function strip(s: string): string {
  return s.replace(ansiRegex(), "");
}
