/// <reference path="deno.d.ts" />
import { deno as pb } from "./msg.pb";
import * as ts from "typescript";

const globalEval = eval;
const window = globalEval("this");

window["denoMain"] = () => {
  denoPrint(`ts.version: ${ts.version}`);
  const res = denoPub("startDeno2", emptyArrayBuffer());
  //denoPrint(`after`);
  const resUi8 = new Uint8Array(res);
  denoPrint(`before`);
  const msg = pb.Msg.decode(resUi8);
  denoPrint(`after`);
  const {
    startCwd: cwd,
    startArgv: argv,
    startDebugFlag: debugFlag,
    startMainJs: mainJs,
    startMainMap: mainMap
  } = msg;

  denoPrint(`cwd: ${cwd}`);
  denoPrint(`debugFlag: ${debugFlag}`);

  for (let i = 0; i < argv.length; i++) {
    denoPrint(`argv[${i}] ${argv[i]}`);
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
