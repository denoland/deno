// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "deno_dispatch_json";
import { opNamespace } from "./namespace.ts";

const OP_CWD = new JsonOp(opNamespace, "cwd");

const OP_CHDIR = new JsonOp(opNamespace, "chdir");

/**
 * `cwd()` Return a string representing the current working directory.
 * If the current directory can be reached via multiple paths
 * (due to symbolic links), `cwd()` may return
 * any one of them.
 * throws `NotFound` exception if directory not available
 */
export function cwd(): string {
  return OP_CWD.sendSync();
}

/**
 * `chdir()` Change the current working directory to path.
 * throws `NotFound` exception if directory not available
 */
export function chdir(directory: string): void {
  OP_CHDIR.sendSync({ directory });
}
