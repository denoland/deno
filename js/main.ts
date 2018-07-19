// tslint:disable-next-line:no-reference
/// <reference path="deno.d.ts" />
import { flatbuffers } from "flatbuffers";
import { deno as fbs } from "./msg_generated";
import { assert, log } from "./util";
import * as runtime from "./runtime";

const globalEval = eval;
const window = globalEval("this");

let cmdIdCounter = 0;
function assignCmdId(): number {
  // TODO(piscisaureus) Safely re-use so they don't overflow.
  const cmdId = ++cmdIdCounter;
  assert(cmdId < 2 ** 32, "cmdId overflow");
  return cmdId;
}

function startMsg(cmdId: number): Uint8Array {
  const builder = new flatbuffers.Builder();
  const msg = fbs.Start.createStart(builder, 0);
  fbs.Base.startBase(builder);
  fbs.Base.addCmdId(builder, cmdId);
  fbs.Base.addMsg(builder, msg);
  fbs.Base.addMsgType(builder, fbs.Any.Start);
  builder.finish(fbs.Base.endBase(builder));
  return builder.asUint8Array();
}

window["denoMain"] = () => {
  // First we send an empty "Start" message to let the privlaged side know we
  // are ready. The response should be a "StartRes" message containing the CLI
  // argv and other info.
  const cmdId = assignCmdId();
  const res = deno.send(startMsg(cmdId));

  // TODO(ry) Remove this conditional once main.rs gets up to speed.
  if (res == null) {
    console.log(`The 'Start' message got a null response.  Normally this would
    be an error but main.rs currently does this."); Exiting without error.`);
    return;
  }

  // Deserialize res into startResMsg.
  const bb = new flatbuffers.ByteBuffer(res);
  const base = fbs.Base.getRootAsBase(bb);
  assert(base.cmdId() === cmdId);
  assert(fbs.Any.StartRes === base.msgType());
  const startResMsg = new fbs.StartRes();
  assert(base.msg(startResMsg) != null);

  const cwd = startResMsg.cwd();
  log("cwd", cwd);

  const argv: string[] = [];
  for (let i = 0; i < startResMsg.argvLength(); i++) {
    argv.push(startResMsg.argv(i));
  }
  log("argv", argv);

  const inputFn = argv[1];
  const mod = runtime.resolveModule(inputFn, `${cwd}/`);
  mod.compileAndRun();
};
