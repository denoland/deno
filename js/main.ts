// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// We need to make sure this module loads, for its side effects.
import "./globals";

import * as flatbuffers from "./flatbuffers";
import * as msg from "gen/msg_generated";
import { assert, log, setLogDebug } from "./util";
import * as os from "./os";
import { DenoCompiler } from "./compiler";
import { libdeno } from "./libdeno";
import { args } from "./deno";
import { sendSync, handleAsyncMsgFromRust } from "./dispatch";
import { replLoop } from "./repl";
import { version } from "typescript";

const compiler = DenoCompiler.instance();

function sendStart(): msg.StartRes {
  const builder = flatbuffers.createBuilder();
  msg.Start.startStart(builder);
  const startOffset = msg.Start.endStart(builder);
  const baseRes = sendSync(builder, msg.Any.Start, startOffset);
  assert(baseRes != null);
  assert(msg.Any.StartRes === baseRes!.innerType());
  const startRes = new msg.StartRes();
  assert(baseRes!.inner(startRes) != null);
  return startRes;
}

/* tslint:disable-next-line:no-default-export */
export default function denoMain() {
  libdeno.recv(handleAsyncMsgFromRust);

  // First we send an empty "Start" message to let the privileged side know we
  // are ready. The response should be a "StartRes" message containing the CLI
  // args and other info.
  const startResMsg = sendStart();

  setLogDebug(startResMsg.debugFlag());

  // handle `--types`
  if (startResMsg.typesFlag()) {
    const defaultLibFileName = compiler.getDefaultLibFileName();
    const defaultLibModule = compiler.resolveModule(defaultLibFileName, "");
    console.log(defaultLibModule.sourceCode);
    os.exit(0);
  }

  // handle `--version`
  if (startResMsg.versionFlag()) {
    console.log("deno:", startResMsg.denoVersion());
    console.log("v8:", startResMsg.v8Version());
    console.log("typescript:", version);
    os.exit(0);
  }

  os.setPid(startResMsg.pid());

  const cwd = startResMsg.cwd();
  log("cwd", cwd);

  for (let i = 1; i < startResMsg.argvLength(); i++) {
    args.push(startResMsg.argv(i));
  }
  log("args", args);
  Object.freeze(args);
  const inputFn = args[0];

  compiler.recompile = startResMsg.recompileFlag();

  if (inputFn) {
    compiler.run(inputFn, `${cwd}/`);
  } else {
    replLoop();
  }
}
