// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { flatbuffers } from "flatbuffers";
import { deno as fbs } from "gen/msg_generated";
import { assert, log, assignCmdId } from "./util";
import * as runtime from "./runtime";
import { libdeno } from "./globals";
import * as timers from "./timers";

function startMsg(cmdId: number): Uint8Array {
  const builder = new flatbuffers.Builder();
  fbs.Start.startStart(builder);
  const startOffset = fbs.Start.endStart(builder);
  fbs.Base.startBase(builder);
  fbs.Base.addCmdId(builder, cmdId);
  fbs.Base.addMsg(builder, startOffset);
  fbs.Base.addMsgType(builder, fbs.Any.Start);
  builder.finish(fbs.Base.endBase(builder));
  return builder.asUint8Array();
}

function onMessage(ui8: Uint8Array) {
  const bb = new flatbuffers.ByteBuffer(ui8);
  const base = fbs.Base.getRootAsBase(bb);
  switch (base.msgType()) {
    case fbs.Any.TimerReady: {
      const msg = new fbs.TimerReady();
      assert(base.msg(msg) != null);
      timers.onMessage(msg);
      break;
    }
    default: {
      assert(false, "Unhandled message type");
      break;
    }
  }
}

/* tslint:disable-next-line:no-default-export */
export default function denoMain() {
  libdeno.recv(onMessage);
  runtime.setup();

  // First we send an empty "Start" message to let the privlaged side know we
  // are ready. The response should be a "StartRes" message containing the CLI
  // argv and other info.
  const cmdId = assignCmdId();
  const res = libdeno.send(startMsg(cmdId));

  // TODO(ry) Remove this conditional once main.rs gets up to speed.
  if (res == null) {
    console.log(`The 'Start' message got a null response.  Normally this would
    be an error but main.rs currently does this."); Exiting without error.`);
    return;
  }

  // Deserialize res into startResMsg.
  const bb = new flatbuffers.ByteBuffer(res);
  const base = fbs.Base.getRootAsBase(bb);
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
}
