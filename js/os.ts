// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { ModuleInfo } from "./types";
import * as fbs from "gen/msg_generated";
import { assert } from "./util";
import * as util from "./util";
import { flatbuffers } from "flatbuffers";
import { sendSync } from "./dispatch";

export function exit(exitCode = 0): never {
  const builder = new flatbuffers.Builder();
  fbs.Exit.startExit(builder);
  fbs.Exit.addCode(builder, exitCode);
  const inner = fbs.Exit.endExit(builder);
  sendSync(builder, fbs.Any.Exit, inner);
  return util.unreachable();
}

export function codeFetch(
  moduleSpecifier: string,
  containingFile: string
): ModuleInfo {
  util.log("os.ts codeFetch", moduleSpecifier, containingFile);
  // Send CodeFetch message
  const builder = new flatbuffers.Builder();
  const moduleSpecifier_ = builder.createString(moduleSpecifier);
  const containingFile_ = builder.createString(containingFile);
  fbs.CodeFetch.startCodeFetch(builder);
  fbs.CodeFetch.addModuleSpecifier(builder, moduleSpecifier_);
  fbs.CodeFetch.addContainingFile(builder, containingFile_);
  const inner = fbs.CodeFetch.endCodeFetch(builder);
  const baseRes = sendSync(builder, fbs.Any.CodeFetch, inner);
  assert(baseRes != null);
  assert(
    fbs.Any.CodeFetchRes === baseRes!.innerType(),
    `base.innerType() unexpectedly is ${baseRes!.innerType()}`
  );
  const codeFetchRes = new fbs.CodeFetchRes();
  assert(baseRes!.inner(codeFetchRes) != null);
  return {
    moduleName: codeFetchRes.moduleName(),
    filename: codeFetchRes.filename(),
    sourceCode: codeFetchRes.sourceCode(),
    outputCode: codeFetchRes.outputCode()
  };
}

export function codeCache(
  filename: string,
  sourceCode: string,
  outputCode: string
): void {
  util.log("os.ts codeCache", filename, sourceCode, outputCode);
  const builder = new flatbuffers.Builder();
  const filename_ = builder.createString(filename);
  const sourceCode_ = builder.createString(sourceCode);
  const outputCode_ = builder.createString(outputCode);
  fbs.CodeCache.startCodeCache(builder);
  fbs.CodeCache.addFilename(builder, filename_);
  fbs.CodeCache.addSourceCode(builder, sourceCode_);
  fbs.CodeCache.addOutputCode(builder, outputCode_);
  const inner = fbs.CodeCache.endCodeCache(builder);
  const baseRes = sendSync(builder, fbs.Any.CodeCache, inner);
  assert(baseRes == null); // Expect null or error.
}

function createEnv(_inner: fbs.EnvironRes): { [index: string]: string } {
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
  const builder = new flatbuffers.Builder();
  const _key = builder.createString(key);
  const _value = builder.createString(value);
  fbs.SetEnv.startSetEnv(builder);
  fbs.SetEnv.addKey(builder, _key);
  fbs.SetEnv.addValue(builder, _value);
  const inner = fbs.SetEnv.endSetEnv(builder);
  sendSync(builder, fbs.Any.SetEnv, inner);
}

/**
 * Returns a snapshot of the environment variables at invocation. Mutating a
 * property in the object will set that variable in the environment for
 * the process. The environment object will only accept `string`s or `number`s
 * as values.
 *     import { env } from "deno";
 *
 *     const myEnv = env();
 *     console.log(myEnv.SHELL);
 *     myEnv.TEST_VAR = "HELLO";
 *     const newEnv = env();
 *     console.log(myEnv.TEST_VAR == newEnv.TEST_VAR);
 */
export function env(): { [index: string]: string } {
  /* Ideally we could write
  const res = sendSync({
    command: fbs.Command.ENV,
  });
  */
  const builder = new flatbuffers.Builder();
  fbs.Environ.startEnviron(builder);
  const inner = fbs.Environ.endEnviron(builder);
  const baseRes = sendSync(builder, fbs.Any.Environ, inner)!;
  assert(fbs.Any.EnvironRes === baseRes.innerType());
  const res = new fbs.EnvironRes();
  assert(baseRes.inner(res) != null);
  // TypeScript cannot track assertion above, therefore not null assertion
  return createEnv(res);
}
