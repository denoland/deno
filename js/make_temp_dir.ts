// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";
import { assert } from "./util";

export interface MakeTempDirOptions {
  dir?: string;
  prefix?: string;
  suffix?: string;
}

function req({
  dir,
  prefix,
  suffix
}: MakeTempDirOptions): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const fbDir = dir == null ? -1 : builder.createString(dir);
  const fbPrefix = prefix == null ? -1 : builder.createString(prefix);
  const fbSuffix = suffix == null ? -1 : builder.createString(suffix);
  msg.MakeTempDir.startMakeTempDir(builder);
  if (dir != null) {
    msg.MakeTempDir.addDir(builder, fbDir);
  }
  if (prefix != null) {
    msg.MakeTempDir.addPrefix(builder, fbPrefix);
  }
  if (suffix != null) {
    msg.MakeTempDir.addSuffix(builder, fbSuffix);
  }
  const inner = msg.MakeTempDir.endMakeTempDir(builder);
  return [builder, msg.Any.MakeTempDir, inner];
}

function res(baseRes: null | msg.Base): string {
  assert(baseRes != null);
  assert(msg.Any.MakeTempDirRes === baseRes!.innerType());
  const res = new msg.MakeTempDirRes();
  assert(baseRes!.inner(res) != null);
  const path = res.path();
  assert(path != null);
  return path!;
}

/** makeTempDirSync is the synchronous version of `makeTempDir`.
 *
 *       const tempDirName0 = Deno.makeTempDirSync();
 *       const tempDirName1 = Deno.makeTempDirSync({ prefix: 'my_temp' });
 */
export function makeTempDirSync(options: MakeTempDirOptions = {}): string {
  return res(dispatch.sendSync(...req(options)));
}

/** makeTempDir creates a new temporary directory in the directory `dir`, its
 * name beginning with `prefix` and ending with `suffix`.
 * It returns the full path to the newly created directory.
 * If `dir` is unspecified, tempDir uses the default directory for temporary
 * files. Multiple programs calling tempDir simultaneously will not choose the
 * same directory. It is the caller's responsibility to remove the directory
 * when no longer needed.
 *
 *       const tempDirName0 = await Deno.makeTempDir();
 *       const tempDirName1 = await Deno.makeTempDir({ prefix: 'my_temp' });
 */
export async function makeTempDir(
  options: MakeTempDirOptions = {}
): Promise<string> {
  return res(await dispatch.sendAsync(...req(options)));
}
