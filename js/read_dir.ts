// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";
import { FileInfo, FileInfoImpl } from "./file_info";
import { assert } from "./util";

/** Reads the directory given by path and returns a list of file info
 * synchronously.
 *
 *       import { readDirSync } from "deno";
 *       const files = readDirSync("/");
 */
export function readDirSync(path: string): FileInfo[] {
  return res(dispatch.sendSync(...req(path)));
}

/** Reads the directory given by path and returns a list of file info.
 *
 *       import { readDir } from "deno";
 *       const files = await readDir("/");
 */
export async function readDir(path: string): Promise<FileInfo[]> {
  return res(await dispatch.sendAsync(...req(path)));
}

function req(path: string): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const path_ = builder.createString(path);
  msg.ReadDir.startReadDir(builder);
  msg.ReadDir.addPath(builder, path_);
  const inner = msg.ReadDir.endReadDir(builder);
  return [builder, msg.Any.ReadDir, inner];
}

function res(baseRes: null | msg.Base): FileInfo[] {
  assert(baseRes != null);
  assert(msg.Any.ReadDirRes === baseRes!.innerType());
  const res = new msg.ReadDirRes();
  assert(baseRes!.inner(res) != null);
  const fileInfos: FileInfo[] = [];
  for (let i = 0; i < res.entriesLength(); i++) {
    fileInfos.push(new FileInfoImpl(res.entries(i)!));
  }
  return fileInfos;
}
