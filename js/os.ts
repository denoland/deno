// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { ModuleInfo } from "./types";
import * as msg from "gen/msg_generated";
import { assert } from "./util";
import * as util from "./util";
import * as flatbuffers from "./flatbuffers";
import { sendSync } from "./dispatch";

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
export function codeFetch(
  moduleSpecifier: string,
  containingFile: string
): ModuleInfo {
  util.log("os.ts codeFetch", moduleSpecifier, containingFile);
  // Send CodeFetch message
  const builder = flatbuffers.createBuilder();
  const moduleSpecifier_ = builder.createString(moduleSpecifier);
  const containingFile_ = builder.createString(containingFile);
  msg.CodeFetch.startCodeFetch(builder);
  msg.CodeFetch.addModuleSpecifier(builder, moduleSpecifier_);
  msg.CodeFetch.addContainingFile(builder, containingFile_);
  const inner = msg.CodeFetch.endCodeFetch(builder);
  const baseRes = sendSync(builder, msg.Any.CodeFetch, inner);
  assert(baseRes != null);
  assert(
    msg.Any.CodeFetchRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const codeFetchRes = new msg.CodeFetchRes();
  assert(baseRes!.inner(codeFetchRes) != null);
  return {
    moduleName: codeFetchRes.moduleName(),
    filename: codeFetchRes.filename(),
    sourceCode: codeFetchRes.sourceCode(),
    outputCode: codeFetchRes.outputCode()
  };
}

// @internal
export function codeCache(
  filename: string,
  sourceCode: string,
  outputCode: string
): void {
  util.log("os.ts codeCache", filename, sourceCode, outputCode);
  const builder = flatbuffers.createBuilder();
  const filename_ = builder.createString(filename);
  const sourceCode_ = builder.createString(sourceCode);
  const outputCode_ = builder.createString(outputCode);
  msg.CodeCache.startCodeCache(builder);
  msg.CodeCache.addFilename(builder, filename_);
  msg.CodeCache.addSourceCode(builder, sourceCode_);
  msg.CodeCache.addOutputCode(builder, outputCode_);
  const inner = msg.CodeCache.endCodeCache(builder);
  const baseRes = sendSync(builder, msg.Any.CodeCache, inner);
  assert(baseRes == null); // Expect null or error.
}

function createEnv(_inner: msg.EnvironRes): { [index: string]: string } {
  const env: { [index: string]: string } = {};

  for (let i = 0; i < _inner.mapLength(); i++) {
    const item = _inner.map(i)!;

    env[item.key()!] = item.value()!;
  }

  return new Proxy(env, {
    set(obj, prop: string, value: string | number) {
      setEnv(prop, value.toString());
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
 * the process. The environment object will only accept `string`s or `number`s
 * as values.
 *
 *       import { env } from "deno";
 *
 *       const myEnv = env();
 *       console.log(myEnv.SHELL);
 *       myEnv.TEST_VAR = "HELLO";
 *       const newEnv = env();
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
