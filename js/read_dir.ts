// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";
import { FileInfo, FileInfoImpl } from "./file_info";
import { assert } from "./util";

function req(path: string): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const path_ = builder.createString(path);
  const inner = msg.ReadDir.createReadDir(builder, path_);
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

/** Reads the directory given by path and returns a list of file info
 * synchronously.
 *
 *       const files = Deno.readDirSync("/");
 */
export function readDirSync(path: string): FileInfo[] {
  return res(dispatch.sendSync(...req(path)));
}

/** Reads the directory given by path and returns a list of file info.
 *
 *       const files = await Deno.readDir("/");
 */
export async function readDir(path: string): Promise<FileInfo[]> {
  return res(await dispatch.sendAsync(...req(path)));
}
