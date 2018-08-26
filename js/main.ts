// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { flatbuffers } from "flatbuffers";
import { deno as fbs } from "gen/msg_generated";
import { assert, assignCmdId, log, setLogDebug } from "./util";
import * as os from "./os";
import { DenoCompiler } from "./compiler";
import { libdeno } from "./libdeno";
import * as timers from "./timers";
import { onFetchRes } from "./fetch";

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
    case fbs.Any.FetchRes: {
      const msg = new fbs.FetchRes();
      assert(base.msg(msg) != null);
      onFetchRes(base, msg);
      break;
    }
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

function onGlobalError(
  message: string,
  source: string,
  lineno: number,
  colno: number,
  error: Error
) {
  console.log(error.stack);
  os.exit(1);
}

/* tslint:disable-next-line:no-default-export */
export default function denoMain() {
  libdeno.recv(onMessage);
  libdeno.setGlobalErrorHandler(onGlobalError);
  const compiler = DenoCompiler.instance();

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

  setLogDebug(startResMsg.debugFlag());

  const cwd = startResMsg.cwd();
  log("cwd", cwd);

  const argv: string[] = [];
  for (let i = 0; i < startResMsg.argvLength(); i++) {
    argv.push(startResMsg.argv(i));
  }
  log("argv", argv);

  const inputFn = argv[1];
  if (!inputFn) {
    console.log("No input script specified.");
    os.exit(1);
  }

  compiler.run(inputFn, `${cwd}/`);
}
