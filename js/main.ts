// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as flatbuffers from "./flatbuffers";
import * as msg from "gen/msg_generated";
import { assert, log, setLogDebug } from "./util";
import * as os from "./os";
import { DenoCompiler } from "./compiler";
import { libdeno } from "./libdeno";
import { args } from "./deno";
import { sendSync, handleAsyncMsgFromRust } from "./dispatch";
import { readFile } from "./read_file";
import { promiseErrorExaminer, promiseRejectHandler } from "./promise_util";
import { version } from "typescript";
import { TextDecoder } from "text-encoding";
import { DenoError } from "./errors";

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

function onGlobalError(
  message: string,
  source: string,
  lineno: number,
  colno: number,
  error: any // tslint:disable-line:no-any
) {
  if (error instanceof Error) {
    console.log(error.stack);
  } else {
    console.log(`Thrown: ${String(error)}`);
  }
  os.exit(1);
}

/* tslint:disable-next-line:no-default-export */
export default async function denoMain() {
  libdeno.recv(handleAsyncMsgFromRust);
  libdeno.setGlobalErrorHandler(onGlobalError);
  libdeno.setPromiseRejectHandler(promiseRejectHandler);
  libdeno.setPromiseErrorExaminer(promiseErrorExaminer);
  const compiler = DenoCompiler.instance();

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

  const cwd = startResMsg.cwd();
  log("cwd", cwd);

  // TODO handle shebang.
  for (let i = 1; i < startResMsg.argvLength(); i++) {
    args.push(startResMsg.argv(i));
  }
  log("args", args);
  Object.freeze(args);

  const inputFn = args[0];
  if (!inputFn) {
    console.log("No input script specified.");
    os.exit(1);
  }

  // handle `--config FILE`
  const tsconfigFilename = startResMsg.configFile();
  if (tsconfigFilename) {
    try {
      const decoder = new TextDecoder("utf-8");
      const tsconfigJson = decoder.decode(await readFile(tsconfigFilename));
      const ignoredOptions = compiler.configure(tsconfigJson);
      if (ignoredOptions) {
        console.warn(
          `Unsupported compiler options in: "${tsconfigFilename}"\n` +
            "  The following options were ignored:\n" +
            `    ${ignoredOptions.join(", ")}`
        );
      }
    } catch (e) {
      if (e instanceof DenoError && e.kind === msg.ErrorKind.NotFound) {
        console.error(
          `Specified configuration "${tsconfigFilename}" not found.`
        );
        os.exit(1);
      } else {
        throw e;
      }
    }
  }

  compiler.recompile = startResMsg.recompileFlag();
  compiler.run(inputFn, `${cwd}/`);
}
