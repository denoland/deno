// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";
import { assert } from "./util";
import { FileInfo, FileInfoImpl } from "./file_info";

function req(
  filename: string,
  lstat: boolean
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const filename_ = builder.createString(filename);
  const inner = msg.Stat.createStat(builder, filename_, lstat);
  return [builder, msg.Any.Stat, inner];
}

function res(baseRes: null | msg.Base): FileInfo {
  assert(baseRes != null);
  assert(msg.Any.StatRes === baseRes!.innerType());
  const res = new msg.StatRes();
  assert(baseRes!.inner(res) != null);
  return new FileInfoImpl(res);
}

/** Queries the file system for information on the path provided. If the given
 * path is a symlink information about the symlink will be returned.
 *
 *       const fileInfo = await Deno.lstat("hello.txt");
 *       assert(fileInfo.isFile());
 */
export async function lstat(filename: string): Promise<FileInfo> {
  return res(await dispatch.sendAsync(...req(filename, true)));
}

/** Queries the file system for information on the path provided synchronously.
 * If the given path is a symlink information about the symlink will be
 * returned.
 *
 *       const fileInfo = Deno.lstatSync("hello.txt");
 *       assert(fileInfo.isFile());
 */
export function lstatSync(filename: string): FileInfo {
  return res(dispatch.sendSync(...req(filename, true)));
}

/** Queries the file system for information on the path provided. `stat` Will
 * always follow symlinks.
 *
 *       const fileInfo = await Deno.stat("hello.txt");
 *       assert(fileInfo.isFile());
 */
export async function stat(filename: string): Promise<FileInfo> {
  return res(await dispatch.sendAsync(...req(filename, false)));
}

/** Queries the file system for information on the path provided synchronously.
 * `statSync` Will always follow symlinks.
 *
 *       const fileInfo = Deno.statSync("hello.txt");
 *       assert(fileInfo.isFile());
 */
export function statSync(filename: string): FileInfo {
  return res(dispatch.sendSync(...req(filename, false)));
}
