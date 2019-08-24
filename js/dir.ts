// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { assert } from "./util";
import { sendSync, flatbuffers, msg } from "./dispatch_flatbuffers";
import { sendSync as sendSyncJson } from "./dispatch_json";
import * as dispatch from "./dispatch";

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
  sendSyncJson(dispatch.OP_CHDIR, { directory });
}
