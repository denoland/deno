// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// Public deno module.
export {
  env,
  exit,
  FileInfo,
  makeTempDirSync,
  readFileSync,
  statSync,
  lStatSync,
  writeFileSync
} from "./os";
export { libdeno } from "./libdeno";
export const argv: string[] = [];
