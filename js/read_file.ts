// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";
import * as dispatch from "./dispatch";

function req(
  filename: string
): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const filename_ = builder.createString(filename);
  msg.ReadFile.startReadFile(builder);
  msg.ReadFile.addFilename(builder, filename_);
  const inner = msg.ReadFile.endReadFile(builder);
  return [builder, msg.Any.ReadFile, inner];
}

function res(baseRes: null | msg.Base): Uint8Array {
  assert(baseRes != null);
  assert(msg.Any.ReadFileRes === baseRes!.innerType());
  const inner = new msg.ReadFileRes();
  assert(baseRes!.inner(inner) != null);
  const dataArray = inner.dataArray();
  assert(dataArray != null);
  return new Uint8Array(dataArray!);
}

/** Read the entire contents of a file synchronously.
 *
 *       const decoder = new TextDecoder("utf-8");
 *       const data = Deno.readFileSync("hello.txt");
 *       console.log(decoder.decode(data));
 */
export function readFileSync(filename: string): Uint8Array {
  return res(dispatch.sendSync(...req(filename)));
}

/** Read the entire contents of a file.
 *
 *       const decoder = new TextDecoder("utf-8");
 *       const data = await Deno.readFile("hello.txt");
 *       console.log(decoder.decode(data));
 */
export async function readFile(filename: string): Promise<Uint8Array> {
  return res(await dispatch.sendAsync(...req(filename)));
}
