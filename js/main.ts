// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import "./globals";

import * as flatbuffers from "./flatbuffers";
import * as msg from "gen/msg_generated";
import { assert, log, setLogDebug } from "./util";
import * as os from "./os";
import { libdeno } from "./libdeno";
import { args } from "./deno";
import { sendSync, handleAsyncMsgFromRust } from "./dispatch";
import { replLoop } from "./repl";

// builtin modules
import * as deno from "./deno";

// Global reference to StartRes so it can be shared between compilerMain and
// denoMain.
let startResMsg: msg.StartRes;

function sendStart(): void {
  const builder = flatbuffers.createBuilder();
  msg.Start.startStart(builder);
  const startOffset = msg.Start.endStart(builder);
  const baseRes = sendSync(builder, msg.Any.Start, startOffset);
  assert(baseRes != null);
  assert(msg.Any.StartRes === baseRes!.innerType());
  startResMsg = new msg.StartRes();
  assert(baseRes!.inner(startResMsg) != null);
}

/* tslint:disable-next-line:no-default-export */
export default function denoMain() {
  libdeno.recv(handleAsyncMsgFromRust);

  libdeno.builtinModules["deno"] = deno;
  // libdeno.builtinModules["typescript"] = typescript;
  Object.freeze(libdeno.builtinModules);

  // First we send an empty "Start" message to let the privileged side know we
  // are ready. The response should be a "StartRes" message containing the CLI
  // args and other info.
  sendStart();

  setLogDebug(startResMsg.debugFlag());

  // handle `--version`
  if (startResMsg.versionFlag()) {
    console.log("deno:", startResMsg.denoVersion());
    console.log("v8:", startResMsg.v8Version());
    // console.log("typescript:", version);
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
  if (!inputFn) {
    replLoop();
  }
}
