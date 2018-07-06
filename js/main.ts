// tslint:disable-next-line:no-reference
/// <reference path="deno.d.ts" />
import * as ts from "typescript";

import { flatbuffers } from "flatbuffers";
import { deno as fbs } from "./msg_generated";
import { assert } from "./util";

// import * as runtime from "./runtime";

const globalEval = eval;
const window = globalEval("this");

function startMsg(): ArrayBuffer {
  const builder = new flatbuffers.Builder();
  const msg = fbs.Start.createStart(builder, 0);
  fbs.Base.startBase(builder);
  fbs.Base.addMsg(builder, msg);
  fbs.Base.addMsgType(builder, fbs.Any.Start);
  builder.finish(fbs.Base.endBase(builder));
  return typedArrayToArrayBuffer(builder.asUint8Array());
}

window["denoMain"] = () => {
  deno.print(`ts.version: ${ts.version}`);

  // First we send an empty "Start" message to let the privlaged side know we
  // are ready. The response should be a "StartRes" message containing the CLI
  // argv and other info.
  const res = deno.send("start", startMsg());

  // TODO(ry) Remove this conditional once main.rs gets up to speed.
  if (res == null) {
    console.log(`The 'Start' message got a null response.  Normally this would
    be an error but main.rs currently does this."); Exiting without error.`);
    return;
  }

  // Deserialize res into startResMsg.
  const bb = new flatbuffers.ByteBuffer(new Uint8Array(res));
  const base = fbs.Base.getRootAsBase(bb);
  assert(fbs.Any.StartRes === base.msgType());
  const startResMsg = new fbs.StartRes();
  assert(base.msg(startResMsg) != null);

  const cwd = startResMsg.cwd();
  deno.print(`cwd: ${cwd}`);

  const argv: string[] = [];
  for (let i = 0; i < startResMsg.argvLength(); i++) {
    argv.push(startResMsg.argv(i));
  }
  deno.print(`argv ${argv}`);

  /* TODO(ry) Uncomment to test further message passing.
  const inputFn = argv[0];
  const mod = runtime.resolveModule(inputFn, `${cwd}/`);
  mod.compileAndRun();
  */
};

function typedArrayToArrayBuffer(ta: Uint8Array): ArrayBuffer {
  return ta.buffer.slice(
    ta.byteOffset,
    ta.byteOffset + ta.byteLength
  ) as ArrayBuffer;
}
