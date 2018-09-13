// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as fbs from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import * as dispatch from "./dispatch";
import { assert } from "./util";

export interface MakeTempDirOptions {
  dir?: string;
  prefix?: string;
  suffix?: string;
}

/**
 * makeTempDirSync is the synchronous version of `makeTempDir`.
 *
 *     import { makeTempDirSync } from "deno";
 *     const tempDirName0 = makeTempDirSync();
 *     const tempDirName1 = makeTempDirSync({ prefix: 'my_temp' });
 */
export function makeTempDirSync(options: MakeTempDirOptions = {}): string {
  return res(dispatch.sendSync(...req(options)));
}

/**
 * makeTempDir creates a new temporary directory in the directory `dir`, its
 * name beginning with `prefix` and ending with `suffix`.
 * It returns the full path to the newly created directory.
 * If `dir` is unspecified, tempDir uses the default directory for temporary
 * files. Multiple programs calling tempDir simultaneously will not choose the
 * same directory. It is the caller's responsibility to remove the directory
 * when no longer needed.
 *
 *     import { makeTempDir } from "deno";
 *     const tempDirName0 = await makeTempDir();
 *     const tempDirName1 = await makeTempDir({ prefix: 'my_temp' });
 */
export async function makeTempDir(
  options: MakeTempDirOptions = {}
): Promise<string> {
  return res(await dispatch.sendAsync(...req(options)));
}

function req({
  dir,
  prefix,
  suffix
}: MakeTempDirOptions): [flatbuffers.Builder, fbs.Any, flatbuffers.Offset] {
  const builder = new flatbuffers.Builder();
  const fbDir = dir == null ? -1 : builder.createString(dir);
  const fbPrefix = prefix == null ? -1 : builder.createString(prefix);
  const fbSuffix = suffix == null ? -1 : builder.createString(suffix);
  fbs.MakeTempDir.startMakeTempDir(builder);
  if (dir != null) {
    fbs.MakeTempDir.addDir(builder, fbDir);
  }
  if (prefix != null) {
    fbs.MakeTempDir.addPrefix(builder, fbPrefix);
  }
  if (suffix != null) {
    fbs.MakeTempDir.addSuffix(builder, fbSuffix);
  }
  const msg = fbs.MakeTempDir.endMakeTempDir(builder);
  return [builder, fbs.Any.MakeTempDir, msg];
}

function res(baseRes: null | fbs.Base): string {
  assert(baseRes != null);
  assert(fbs.Any.MakeTempDirRes === baseRes!.msgType());
  const res = new fbs.MakeTempDirRes();
  assert(baseRes!.msg(res) != null);
  const path = res.path();
  assert(path != null);
  return path!;
}
