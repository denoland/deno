// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// Public deno module.
/// <amd-module name="deno"/>
export {
  env,
  exit,
  FileInfo,
  makeTempDirSync,
  mkdirSync,
  renameSync,
  statSync,
  lstatSync,
  writeFileSync
} from "./os";
export { readFileSync, readFile } from "./read_file";
export { ErrorKind, DenoError } from "./errors";
export { libdeno } from "./libdeno";
export const argv: string[] = [];
