// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";
import { assert } from "./util";
import { FileInfo, FileInfoImpl } from "./file_info";

/** Queries the file system for information on the path provided. If the given
 * path is a symlink information about the symlink will be returned.
 *
 *       import { lstat } from "deno";
 *       const fileInfo = await lstat("hello.txt");
 *       assert(fileInfo.isFile());
 */
export async function lstat(filename: string): Promise<FileInfo> {
  return res(await dispatch.sendAsync(...req(filename, true)));
}

/** Queries the file system for information on the path provided synchronously.
 * If the given path is a symlink information about the symlink will be
 * returned.
 *
 *       import { lstatSync } from "deno";
 *       const fileInfo = lstatSync("hello.txt");
 *       assert(fileInfo.isFile());
 */
export function lstatSync(filename: string): FileInfo {
  return res(dispatch.sendSync(...req(filename, true)));
}

/** Queries the file system for information on the path provided. `stat` Will
 * always follow symlinks.
 *
 *       import { stat } from "deno";
 *       const fileInfo = await stat("hello.txt");
 *       assert(fileInfo.isFile());
 */
export async function stat(filename: string): Promise<FileInfo> {
  return res(await dispatch.sendAsync(...req(filename, false)));
}

/** Queries the file system for information on the path provided synchronously.
 * `statSync` Will always follow symlinks.
 *
 *       import { statSync } from "deno";
 *       const fileInfo = statSync("hello.txt");
 *       assert(fileInfo.isFile());
 */
export function statSync(filename: string): FileInfo {
  return res(dispatch.sendSync(...req(filename, false)));
}

function req(
  filename: string,
  lstat: boolean
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const filename_ = builder.createString(filename);
  msg.Stat.startStat(builder);
  msg.Stat.addFilename(builder, filename_);
  msg.Stat.addLstat(builder, lstat);
  const inner = msg.Stat.endStat(builder);
  return [builder, msg.Any.Stat, inner];
}

function res(baseRes: null | msg.Base): FileInfo {
  assert(baseRes != null);
  assert(msg.Any.StatRes === baseRes!.innerType());
  const res = new msg.StatRes();
  assert(baseRes!.inner(res) != null);
  return new FileInfoImpl(res);
}
