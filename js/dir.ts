// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import { assert } from "./util";
import * as flatbuffers from "./flatbuffers";
import { sendSync } from "./dispatch";

/**
 * cwd() Return a string representing the current working directory.
 * If the current directory can be reached via multiple paths
 * (due to symbolic links), cwd() may return
 * any one of them.
 * throws NotFound exception if directory not available
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
 * chdir() Change the current working directory to path.
 * throws NotFound exception if directory not available
 */
export function chdir(directory: string): void {
  const builder = flatbuffers.createBuilder();
  const directory_ = builder.createString(directory);
  msg.Chdir.startChdir(builder);
  msg.Chdir.addDirectory(builder, directory_);
  const inner = msg.Chdir.endChdir(builder);
  sendSync(builder, msg.Any.Chdir, inner);
}
