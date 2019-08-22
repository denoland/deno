// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { assert } from "./util";
import { sendSync, flatbuffers, msg } from "./dispatch_flatbuffers";

/**
 * `cwd()` Return a string representing the current working directory.
 * If the current directory can be reached via multiple paths
 * (due to symbolic links), `cwd()` may return
 * any one of them.
 * throws `NotFound` exception if directory not available
 */
export function cwd(): string {
  const builder = flatbuffers.createBuilder();
  msg.Cwd.startCwd(builder);
  const inner = msg.Cwd.endCwd(builder);
  const baseRes = sendSync(builder, msg.Any.Cwd, inner);
  assert(baseRes != null);
  assert(msg.Any.CwdRes === baseRes!.innerType());
  const res = new msg.CwdRes();
  assert(baseRes!.inner(res) != null);
  return res.cwd()!;
}

/**
 * `chdir()` Change the current working directory to path.
 * throws `NotFound` exception if directory not available
 */
export function chdir(directory: string): void {
  const builder = flatbuffers.createBuilder();
  const directory_ = builder.createString(directory);
  const inner = msg.Chdir.createChdir(builder, directory_);
  sendSync(builder, msg.Any.Chdir, inner);
}
