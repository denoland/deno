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
  const msg = fbs.Exit.endExit(builder);
  sendSync(builder, fbs.Any.Exit, msg);
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
  const msg = fbs.CodeFetch.endCodeFetch(builder);
  const baseRes = sendSync(builder, fbs.Any.CodeFetch, msg);
  assert(baseRes != null);
  assert(
    fbs.Any.CodeFetchRes === baseRes!.msgType(),
    `base.msgType() unexpectedly is ${baseRes!.msgType()}`
  );
  const codeFetchRes = new fbs.CodeFetchRes();
  assert(baseRes!.msg(codeFetchRes) != null);
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
  const msg = fbs.CodeCache.endCodeCache(builder);
  const baseRes = sendSync(builder, fbs.Any.CodeCache, msg);
  assert(baseRes == null); // Expect null or error.
}

/**
 * makeTempDirSync creates a new temporary directory in the directory `dir`, its
 * name beginning with `prefix` and ending with `suffix`.
 * It returns the full path to the newly created directory.
 * If `dir` is unspecified, tempDir uses the default directory for temporary
 * files. Multiple programs calling tempDir simultaneously will not choose the
 * same directory. It is the caller's responsibility to remove the directory
 * when no longer needed.
 */
export interface MakeTempDirOptions {
  dir?: string;
  prefix?: string;
  suffix?: string;
}
export function makeTempDirSync({
  dir,
  prefix,
  suffix
}: MakeTempDirOptions = {}): string {
  const builder = new flatbuffers.Builder();
  const fbDir = dir == null ? -1 : builder.createString(dir);
  const fbPrefix = prefix == null ? -1 : builder.createString(prefix);
  const fbSuffix = suffix == null ? -1 : builder.createString(suffix);
  fbs.MakeTempDir.startMakeTempDir(builder);
  if (dir != null) {
    fbs.MakeTempDir.addDir(builder, fbDir);
  }
  if (prefix != null) {
    fbs.MakeTempDir.addPrefix(builder, fbPrefix);
  }
  if (suffix != null) {
    fbs.MakeTempDir.addSuffix(builder, fbSuffix);
  }
  const msg = fbs.MakeTempDir.endMakeTempDir(builder);
  const baseRes = sendSync(builder, fbs.Any.MakeTempDir, msg);
  assert(baseRes != null);
  assert(fbs.Any.MakeTempDirRes === baseRes!.msgType());
  const res = new fbs.MakeTempDirRes();
  assert(baseRes!.msg(res) != null);
  const path = res.path();
  assert(path != null);
  return path!;
}

function createEnv(_msg: fbs.EnvironRes): { [index: string]: string } {
  const env: { [index: string]: string } = {};

  for (let i = 0; i < _msg.mapLength(); i++) {
    const item = _msg.map(i)!;

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
  const msg = fbs.SetEnv.endSetEnv(builder);
  sendSync(builder, fbs.Any.SetEnv, msg);
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
  const msg = fbs.Environ.endEnviron(builder);
  const baseRes = sendSync(builder, fbs.Any.Environ, msg)!;
  assert(fbs.Any.EnvironRes === baseRes.msgType());
  const res = new fbs.EnvironRes();
  assert(baseRes.msg(res) != null);
  // TypeScript cannot track assertion above, therefore not null assertion
  return createEnv(res);
}

/**
 * Renames (moves) oldpath to newpath.
 *     import { renameSync } from "deno";
 *     const oldpath = 'from/path';
 *     const newpath = 'to/path';
 *
 *     renameSync(oldpath, newpath);
 */
export function renameSync(oldpath: string, newpath: string): void {
  const builder = new flatbuffers.Builder();
  const _oldpath = builder.createString(oldpath);
  const _newpath = builder.createString(newpath);
  fbs.RenameSync.startRenameSync(builder);
  fbs.RenameSync.addOldpath(builder, _oldpath);
  fbs.RenameSync.addNewpath(builder, _newpath);
  const msg = fbs.RenameSync.endRenameSync(builder);
  sendSync(builder, fbs.Any.RenameSync, msg);
}
