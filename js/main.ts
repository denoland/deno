// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { window } from "./globals";

import * as flatbuffers from "./flatbuffers";
import * as msg from "gen/msg_generated";
import { assert, log, setLogDebug } from "./util";
import * as os from "./os";
import { Compiler } from "./compiler";
import { libdeno } from "./libdeno";
import { args } from "./deno";
import { sendSync, handleAsyncMsgFromRust } from "./dispatch";
import { replLoop } from "./repl";
import { version } from "typescript";
import { postMessage } from "./workers";
import { TextDecoder, TextEncoder } from "./text_encoding";
import { ModuleSpecifier, ContainingFile } from "./compiler";

// builtin modules
import * as deno from "./deno";

type CompilerLookup = { specifier: ModuleSpecifier; referrer: ContainingFile };

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

function compilerMain() {
  // workerMain should have already been called since a compiler is a worker.
  const compiler = Compiler.instance();
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  compiler.recompile = startResMsg.recompileFlag();
  log(`recompile ${compiler.recompile}`);
  window.onmessage = ({ data }: { data: Uint8Array }) => {
    const json = decoder.decode(data);
    const lookup = JSON.parse(json) as CompilerLookup;

    const moduleMetaData = compiler.compile(lookup.specifier, lookup.referrer);

    const responseJson = JSON.stringify(moduleMetaData);
    const response = encoder.encode(responseJson);
    postMessage(response);
  };
}
window["compilerMain"] = compilerMain;

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

  // handle `--types`
  // TODO(kitsonk) move to Rust fetching from compiler
  if (startResMsg.typesFlag()) {
    const compiler = Compiler.instance();
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
  if (!inputFn) {
    replLoop();
  }
}
