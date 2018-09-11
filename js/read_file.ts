// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as fbs from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import { assert } from "./util";
import * as dispatch from "./dispatch";

/**
 * Read the entire contents of a file synchronously.
 *
 *     import { readFileSync } from "deno";
 *     const decoder = new TextDecoder("utf-8");
 *     const data = readFileSync("hello.txt");
 *     console.log(decoder.decode(data));
 */
export function readFileSync(filename: string): Uint8Array {
  return res(dispatch.sendSync(...req(filename)));
}

/**
 * Read the entire contents of a file.
 *
 *     import { readFile } from "deno";
 *     const decoder = new TextDecoder("utf-8");
 *     const data = await readFile("hello.txt");
 *     console.log(decoder.decode(data));
 */
export async function readFile(filename: string): Promise<Uint8Array> {
  return res(await dispatch.sendAsync(...req(filename)));
}

function req(
  filename: string
): [flatbuffers.Builder, fbs.Any, flatbuffers.Offset] {
  const builder = new flatbuffers.Builder();
  const filename_ = builder.createString(filename);
  fbs.ReadFile.startReadFile(builder);
  fbs.ReadFile.addFilename(builder, filename_);
  const msg = fbs.ReadFile.endReadFile(builder);
  return [builder, fbs.Any.ReadFile, msg];
}

function res(baseRes: null | fbs.Base): Uint8Array {
  assert(baseRes != null);
  assert(fbs.Any.ReadFileRes === baseRes!.msgType());
  const msg = new fbs.ReadFileRes();
  assert(baseRes!.msg(msg) != null);
  const dataArray = msg.dataArray();
  assert(dataArray != null);
  return new Uint8Array(dataArray!);
}
