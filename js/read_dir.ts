// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as fbs from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import * as dispatch from "./dispatch";
import { FileInfo } from "./stat";
import { assert } from "./util";

/**
 * Queries the file system for information on the path provided.
 * `stat` Will always follow symlinks.
 *
 *     import { stat } from "deno";
 *     const fileInfo = await deno.stat("hello.txt");
 *     assert(fileInfo.isFile());
 */
export async function readDir(filename: string): Promise<FileInfo[]> {
  return res(await dispatch.sendAsync(...req(filename)));
}

/**
 * Queries the file system for information on the path provided synchronously.
 * `statSync` Will always follow symlinks.
 *
 *     import { statSync } from "deno";
 *     const fileInfo = deno.statSync("hello.txt");
 *     assert(fileInfo.isFile());
 */
export function readDirSync(filename: string): FileInfo[] {
  return res(dispatch.sendSync(...req(filename)));
}

function req(
  filename: string,
): [flatbuffers.Builder, fbs.Any, flatbuffers.Offset] {
  const builder = new flatbuffers.Builder();
  const filename_ = builder.createString(filename);
  fbs.ReadDir.startReadDir(builder);
  fbs.ReadDir.addFilename(builder, filename_);
  const msg = fbs.ReadDir.endReadDir(builder);
  return [builder, fbs.Any.ReadDir, msg];
}

function res(baseRes: null | fbs.Base): FileInfo[] {
  assert(baseRes != null);
  assert(fbs.Any.ReadDirRes === baseRes!.msgType());
  const res = new fbs.ReadDirRes();
  assert(baseRes!.msg(res) != null);
  const fileInfos: FileInfo[] = [];

  for (let i = 0; i < res.entriesLength(); i++) {
    fileInfos.push(new FileInfo(res.entries(i)!));
  }

  return fileInfos;
}
