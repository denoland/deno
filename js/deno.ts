// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// Public deno module.
/// <amd-module name="deno"/>
export {
  env,
  exit,
  FileInfo,
  makeTempDirSync,
  mkdirSync,
  readFileSync,
  renameSync,
  statSync,
  lstatSync,
  writeFileSync
} from "./os";
export { ErrorKind, DenoError } from "./errors";
export { libdeno } from "./libdeno";
export const argv: string[] = [];
