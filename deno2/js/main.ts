/// <reference path="deno.d.ts" />
import { deno as pb } from "./msg.pb";
import * as ts from "typescript";

const globalEval = eval;
const window = globalEval("this");

window["denoMain"] = () => {
  deno.print(`ts.version: ${ts.version}`);
  const res = deno.pub("startDeno2", emptyArrayBuffer());
  //deno.print(`after`);
  const resUi8 = new Uint8Array(res);
  deno.print(`before`);
  const msg = pb.Msg.decode(resUi8);
  deno.print(`after`);
  const {
    startCwd: cwd,
    startArgv: argv,
    startDebugFlag: debugFlag,
    startMainJs: mainJs,
    startMainMap: mainMap
  } = msg;

  deno.print(`cwd: ${cwd}`);
  deno.print(`debugFlag: ${debugFlag}`);

  for (let i = 0; i < argv.length; i++) {
    deno.print(`argv[${i}] ${argv[i]}`);
  }
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
