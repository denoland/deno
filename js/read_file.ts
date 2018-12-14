// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import * as dispatch from "./dispatch";
import { assert } from "./util";
import { statSync } from "./stat";
import { open } from "./files";

/** Read the entire contents of a file synchronously.
 *
 *       import { readFileSync } from "deno";
 *       const decoder = new TextDecoder("utf-8");
 *       const data = readFileSync("hello.txt");
 *       console.log(decoder.decode(data));
 */
export function readFileSync(filename: string): Uint8Array {
  const buf = prepared_buf(filename);
  if (buf.length === 0) {
    return buf;
  }
  const builder = flatbuffers.createBuilder();
  const filename_ = builder.createString(filename);
  msg.ReadFile.startReadFile(builder);
  msg.ReadFile.addFilename(builder, filename_);
  const inner = msg.ReadFile.endReadFile(builder);
  dispatch.sendSync(builder, msg.Any.ReadFile, inner, buf);
  return buf;
}

/** Read the entire contents of a file.
 *
 *       import { readFile } from "deno";
 *       const decoder = new TextDecoder("utf-8");
 *       const data = await readFile("hello.txt");
 *       console.log(decoder.decode(data));
 */
export async function readFile(filename: string): Promise<Uint8Array> {
  const buf = prepared_buf(filename);
  if (buf.length === 0) {
    return buf;
  }
  const file = await open(filename, "r");
  await file.read(buf);
  return buf;
}

function prepared_buf(filename: string): Uint8Array {
  const fileInfo = statSync(filename);
  assert(fileInfo.isFile(), "invalid file: " + filename);
  return new Uint8Array(fileInfo.len);
}
