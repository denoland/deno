// tslint:disable-next-line:no-reference
/// <reference path="deno.d.ts" />
import * as ts from "typescript";

import { flatbuffers } from "flatbuffers";
import { deno as fbs } from "./msg_generated";

const globalEval = eval;
const window = globalEval("this");

window["denoMain"] = () => {
  deno.print(`ts.version: ${ts.version}`);
  const res = deno.send("startDeno2", emptyArrayBuffer());
  // deno.print(`after`);
  const resUi8 = new Uint8Array(res);

  const bb = new flatbuffers.ByteBuffer(resUi8);
  const msg = fbs.Msg.getRootAsMsg(bb);

  // startDebugFlag: debugFlag,
  // startMainJs: mainJs,
  // startMainMap: mainMap
  const cwd = msg.startCwd();
  deno.print(`cwd: ${cwd}`);

  const argv: string[] = [];
  for (let i = 0; i < msg.startArgvLength(); i++) {
    argv.push(msg.startArgv(i));
  }
  deno.print(`argv ${argv}`);
};

function typedArrayToArrayBuffer(ta: Uint8Array): ArrayBuffer {
  return ta.buffer.slice(
    ta.byteOffset,
    ta.byteOffset + ta.byteLength
  ) as ArrayBuffer;
}

function emptyArrayBuffer(): ArrayBuffer {
  return typedArrayToArrayBuffer(new Uint8Array([]));
}
