// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { ModuleInfo } from "./types";
import { deno as fbs } from "gen/msg_generated";
import { assert } from "./util";
import * as util from "./util";
import { flatbuffers } from "flatbuffers";
import { libdeno } from "./globals";

export function exit(exitCode = 0): never {
  const builder = new flatbuffers.Builder();
  fbs.Exit.startExit(builder);
  fbs.Exit.addCode(builder, exitCode);
  const msg = fbs.Exit.endExit(builder);
  fbs.Base.startBase(builder);
  fbs.Base.addMsg(builder, msg);
  fbs.Base.addMsgType(builder, fbs.Any.Exit);
  builder.finish(fbs.Base.endBase(builder));
  libdeno.send(builder.asUint8Array());
  throw Error("Unreachable");
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
  fbs.Base.startBase(builder);
  fbs.Base.addMsg(builder, msg);
  fbs.Base.addMsgType(builder, fbs.Any.CodeFetch);
  builder.finish(fbs.Base.endBase(builder));
  const resBuf = libdeno.send(builder.asUint8Array());
  // Process CodeFetchRes
  const bb = new flatbuffers.ByteBuffer(new Uint8Array(resBuf));
  const baseRes = fbs.Base.getRootAsBase(bb);
  if (fbs.Any.NONE === baseRes.msgType()) {
    throw Error(baseRes.error());
  }
  assert(fbs.Any.CodeFetchRes === baseRes.msgType());
  const codeFetchRes = new fbs.CodeFetchRes();
  assert(baseRes.msg(codeFetchRes) != null);
  const r = {
    moduleName: codeFetchRes.moduleName(),
    filename: codeFetchRes.filename(),
    sourceCode: codeFetchRes.sourceCode(),
    outputCode: codeFetchRes.outputCode()
  };
  return r;
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
  fbs.Base.startBase(builder);
  fbs.Base.addMsg(builder, msg);
  fbs.Base.addMsgType(builder, fbs.Any.CodeCache);
  builder.finish(fbs.Base.endBase(builder));
  const resBuf = libdeno.send(builder.asUint8Array());
  // Expect null or error.
  if (resBuf != null) {
    const bb = new flatbuffers.ByteBuffer(new Uint8Array(resBuf));
    const baseRes = fbs.Base.getRootAsBase(bb);
    assert(fbs.Any.NONE === baseRes.msgType());
    throw Error(baseRes.error());
  }
}

export function readFileSync(filename: string): Uint8Array {
  assert(false, "Not Implemented");
  return null;
  /*
  const res = pubInternal("os", {
    command: fbs.Command.READ_FILE_SYNC,
    readFileSyncFilename: filename
  });
  return res.readFileSyncData;
	*/
}

export function writeFileSync(
  filename: string,
  data: Uint8Array,
  perm: number
): void {
  assert(false, "Not Implemented");
  /*
  pubInternal("os", {
    command: fbs.Command.WRITE_FILE_SYNC,
    writeFileSyncFilename: filename,
    writeFileSyncData: data,
    writeFileSyncPerm: perm
  });
  */
}
