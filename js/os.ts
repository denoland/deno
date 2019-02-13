// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import { handleAsyncMsgFromRust, sendSync } from "./dispatch";
import * as flatbuffers from "./flatbuffers";
import { libdeno } from "./libdeno";
import { assert } from "./util";
import * as util from "./util";

/** The current process id of the runtime. */
export let pid: number;

/** Reflects the NO_COLOR environment variable: https://no-color.org/ */
export let noColor: boolean;

/** @internal */
export function setGlobals(pid_: number, noColor_: boolean): void {
  assert(!pid);
  pid = pid_;
  noColor = noColor_;
}

interface CodeInfo {
  moduleName: string | undefined;
  filename: string | undefined;
  mediaType: msg.MediaType;
  sourceCode: string | undefined;
}

/** Check if running in terminal.
 *
 *       console.log(Deno.isTTY().stdout);
 */
export function isTTY(): { stdin: boolean; stdout: boolean; stderr: boolean } {
  const builder = flatbuffers.createBuilder();
  msg.IsTTY.startIsTTY(builder);
  const inner = msg.IsTTY.endIsTTY(builder);
  const baseRes = sendSync(builder, msg.Any.IsTTY, inner)!;
  assert(msg.Any.IsTTYRes === baseRes.innerType());
  const res = new msg.IsTTYRes();
  assert(baseRes.inner(res) != null);

  return { stdin: res.stdin(), stdout: res.stdout(), stderr: res.stderr() };
}

/** Exit the Deno process with optional exit code. */
export function exit(exitCode = 0): never {
  const builder = flatbuffers.createBuilder();
  msg.Exit.startExit(builder);
  msg.Exit.addCode(builder, exitCode);
  const inner = msg.Exit.endExit(builder);
  sendSync(builder, msg.Any.Exit, inner);
  return util.unreachable();
}

// @internal
export function codeFetch(specifier: string, referrer: string): CodeInfo {
  util.log("os.codeFetch", { specifier, referrer });
  // Send CodeFetch message
  const builder = flatbuffers.createBuilder();
  const specifier_ = builder.createString(specifier);
  const referrer_ = builder.createString(referrer);
  msg.CodeFetch.startCodeFetch(builder);
  msg.CodeFetch.addSpecifier(builder, specifier_);
  msg.CodeFetch.addReferrer(builder, referrer_);
  const inner = msg.CodeFetch.endCodeFetch(builder);
  const baseRes = sendSync(builder, msg.Any.CodeFetch, inner);
  assert(baseRes != null);
  assert(
    msg.Any.CodeFetchRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const codeFetchRes = new msg.CodeFetchRes();
  assert(baseRes!.inner(codeFetchRes) != null);
  // flatbuffers returns `null` for an empty value, this does not fit well with
  // idiomatic TypeScript under strict null checks, so converting to `undefined`
  return {
    moduleName: codeFetchRes.moduleName() || undefined,
    filename: codeFetchRes.filename() || undefined,
    mediaType: codeFetchRes.mediaType(),
    sourceCode: codeFetchRes.sourceCode() || undefined
  };
}

// @internal
export function codeCache(
  filename: string,
  sourceCode: string,
  outputCode: string,
  sourceMap: string
): void {
  util.log("os.codeCache", {
    filename,
    sourceCodeLength: sourceCode.length,
    outputCodeLength: outputCode.length,
    sourceMapLength: sourceMap.length
  });
  const builder = flatbuffers.createBuilder();
  const filename_ = builder.createString(filename);
  const sourceCode_ = builder.createString(sourceCode);
  const outputCode_ = builder.createString(outputCode);
  const sourceMap_ = builder.createString(sourceMap);
  msg.CodeCache.startCodeCache(builder);
  msg.CodeCache.addFilename(builder, filename_);
  msg.CodeCache.addSourceCode(builder, sourceCode_);
  msg.CodeCache.addOutputCode(builder, outputCode_);
  msg.CodeCache.addSourceMap(builder, sourceMap_);
  const inner = msg.CodeCache.endCodeCache(builder);
  const baseRes = sendSync(builder, msg.Any.CodeCache, inner);
  assert(baseRes == null); // Expect null or error.
}

function createEnv(inner: msg.EnvironRes): { [index: string]: string } {
  const env: { [index: string]: string } = {};

  for (let i = 0; i < inner.mapLength(); i++) {
    const item = inner.map(i)!;
    env[item.key()!] = item.value()!;
  }

  return new Proxy(env, {
    set(obj, prop: string, value: string) {
      setEnv(prop, value);
      return Reflect.set(obj, prop, value);
    }
  });
}

function setEnv(key: string, value: string): void {
  const builder = flatbuffers.createBuilder();
  const _key = builder.createString(key);
  const _value = builder.createString(value);
  msg.SetEnv.startSetEnv(builder);
  msg.SetEnv.addKey(builder, _key);
  msg.SetEnv.addValue(builder, _value);
  const inner = msg.SetEnv.endSetEnv(builder);
  sendSync(builder, msg.Any.SetEnv, inner);
}

/** Returns a snapshot of the environment variables at invocation. Mutating a
 * property in the object will set that variable in the environment for
 * the process. The environment object will only accept `string`s
 * as values.
 *
 *       const myEnv = Deno.env();
 *       console.log(myEnv.SHELL);
 *       myEnv.TEST_VAR = "HELLO";
 *       const newEnv = Deno.env();
 *       console.log(myEnv.TEST_VAR == newEnv.TEST_VAR);
 */
export function env(): { [index: string]: string } {
  /* Ideally we could write
  const res = sendSync({
    command: msg.Command.ENV,
  });
  */
  const builder = flatbuffers.createBuilder();
  msg.Environ.startEnviron(builder);
  const inner = msg.Environ.endEnviron(builder);
  const baseRes = sendSync(builder, msg.Any.Environ, inner)!;
  assert(msg.Any.EnvironRes === baseRes.innerType());
  const res = new msg.EnvironRes();
  assert(baseRes.inner(res) != null);
  // TypeScript cannot track assertion above, therefore not null assertion
  return createEnv(res);
}

/** Send to the privileged side that we have setup and are ready. */
function sendStart(): msg.StartRes {
  const builder = flatbuffers.createBuilder();
  msg.Start.startStart(builder);
  const startOffset = msg.Start.endStart(builder);
  const baseRes = sendSync(builder, msg.Any.Start, startOffset);
  assert(baseRes != null);
  assert(msg.Any.StartRes === baseRes!.innerType());
  const startResMsg = new msg.StartRes();
  assert(baseRes!.inner(startResMsg) != null);
  return startResMsg;
}

// This function bootstraps an environment within Deno, it is shared both by
// the runtime and the compiler environments.
// @internal
export function start(source?: string): msg.StartRes {
  libdeno.recv(handleAsyncMsgFromRust);

  // First we send an empty `Start` message to let the privileged side know we
  // are ready. The response should be a `StartRes` message containing the CLI
  // args and other info.
  const startResMsg = sendStart();

  util.setLogDebug(startResMsg.debugFlag(), source);

  setGlobals(startResMsg.pid(), startResMsg.noColor());

  return startResMsg;
}
