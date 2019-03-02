import { FileInfo } from "deno";
import { globrex, GlobOptions } from "./globrex.ts";

export function glob(glob: string, options: GlobOptions = {}): RegExp {
  return globrex(glob, options).regex;
}
