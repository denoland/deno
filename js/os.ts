// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
import { ModuleInfo } from "./types";
import { deno as fbs } from "./msg_generated";
import { assert, typedArrayToArrayBuffer  } from "./util";
import { flatbuffers } from "flatbuffers";

export function exit(exitCode = 0): void {
  assert(false, "Not Implemented");
  /*
  pubInternal("os", {
    command: fbs.Command.EXIT,
    exitCode
  });
  */
}

export function codeFetch(
  moduleSpecifier: string,
  containingFile: string
): ModuleInfo {
  console.log("Hello from codeFetch");

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
	const payload = typedArrayToArrayBuffer(builder.asUint8Array());
  const resBuf = deno.send("x", payload);

  console.log("CodeFetch sent");

  // Process CodeFetchRes
  const bb = new flatbuffers.ByteBuffer(new Uint8Array(resBuf));
  const baseRes = fbs.Base.getRootAsBase(bb);
  assert(fbs.Any.CodeFetchRes === baseRes.msgType());
  const codeFetchRes = new fbs.CodeFetchRes();
  assert(baseRes.msg(codeFetchRes) != null);
  return {
    moduleName: codeFetchRes.moduleName(),
    filename: codeFetchRes.filename(),
    sourceCode: codeFetchRes.sourceCode(),
    outputCode: codeFetchRes.outputCode(),
  };
}

export function codeCache(
  filename: string,
  sourceCode: string,
  outputCode: string
): void {
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
  builder.finish(fbs.Base.endBase(builder));

  // Maybe need to do another step?
  // Base.finishBaseBuffer(builder, base);

	const payload = typedArrayToArrayBuffer(builder.asUint8Array());
  const resBuf = deno.send("x", payload);
  assert(resBuf === null);
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
