// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { flatbuffers } from "flatbuffers";
import { deno as fbs } from "gen/msg_generated";
import { assert, log, setLogDebug } from "./util";
import * as os from "./os";
import { DenoCompiler } from "./compiler";
import { libdeno } from "./libdeno";
import { argv } from "./deno";
import { send, handleAsyncMsgFromRust } from "./fbs_util";

function sendStart(): fbs.StartRes {
  const builder = new flatbuffers.Builder();
  fbs.Start.startStart(builder);
  const startOffset = fbs.Start.endStart(builder);
  const baseRes = send(builder, fbs.Any.Start, startOffset);
  assert(baseRes != null);
  assert(fbs.Any.StartRes === baseRes!.msgType());
  const startRes = new fbs.StartRes();
  assert(baseRes!.msg(startRes) != null);
  return startRes;
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
  libdeno.recv(handleAsyncMsgFromRust);
  libdeno.setGlobalErrorHandler(onGlobalError);
  const compiler = DenoCompiler.instance();

  // First we send an empty "Start" message to let the privlaged side know we
  // are ready. The response should be a "StartRes" message containing the CLI
  // argv and other info.
  const startResMsg = sendStart();

  setLogDebug(startResMsg.debugFlag());

  const cwd = startResMsg.cwd();
  log("cwd", cwd);

  // TODO handle shebang.
  for (let i = 1; i < startResMsg.argvLength(); i++) {
    argv.push(startResMsg.argv(i));
  }
  log("argv", argv);
  Object.freeze(argv);

  const inputFn = argv[0];
  if (!inputFn) {
    console.log("No input script specified.");
    os.exit(1);
  }

  const printDeps = startResMsg.depsFlag();
  if (printDeps) {
    for (const dep of compiler.getModuleDependencies(inputFn, `${cwd}/`)) {
      console.log(dep);
    }
    os.exit(0);
  }

  compiler.run(inputFn, `${cwd}/`);
}
