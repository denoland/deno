// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as fbs from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import { assert } from "./util";
import * as dispatch from "./dispatch";

/**
 * Returns the destination of the named symbolic link synchronously.
 *
 *     import { readlinkSync } from "deno";
 *     const targetPath = readlinkSync("symlink/path");
 */
export function readlinkSync(name: string): string {
  return res(dispatch.sendSync(...req(name)));
}

/**
 * Returns the destination of the named symbolic link.
 *
 *     import { readlink } from "deno";
 *     const targetPath = await readlink("symlink/path");
 */
export async function readlink(name: string): Promise<string> {
  return res(await dispatch.sendAsync(...req(name)));
}

function req(name: string): [flatbuffers.Builder, fbs.Any, flatbuffers.Offset] {
  const builder = new flatbuffers.Builder();
  const name_ = builder.createString(name);
  fbs.Readlink.startReadlink(builder);
  fbs.Readlink.addName(builder, name_);
  const inner = fbs.Readlink.endReadlink(builder);
  return [builder, fbs.Any.Readlink, inner];
}

function res(baseRes: null | fbs.Base): string {
  assert(baseRes !== null);
  assert(fbs.Any.ReadlinkRes === baseRes!.innerType());
  const res = new fbs.ReadlinkRes();
  assert(baseRes!.inner(res) !== null);
  const path = res.path();
  assert(path !== null);
  return path!;
}
