// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { ModuleInfo } from "./types";
import { deno as fbs } from "gen/msg_generated";
import { assert } from "./util";
import * as util from "./util";
import { maybeThrowError } from "./errors";
import { flatbuffers } from "flatbuffers";
import { libdeno } from "./libdeno";

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
  fbs.Base.startBase(builder);
  fbs.Base.addMsg(builder, msg);
  fbs.Base.addMsgType(builder, fbs.Any.CodeFetch);
  builder.finish(fbs.Base.endBase(builder));
  const resBuf = libdeno.send(builder.asUint8Array());
  assert(resBuf != null);
  // Process CodeFetchRes
  // TypeScript does not track `assert` from a CFA perspective, therefore not
  // null assertion `!`
  const bb = new flatbuffers.ByteBuffer(new Uint8Array(resBuf!));
  const baseRes = fbs.Base.getRootAsBase(bb);
  maybeThrowError(baseRes);
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
    maybeThrowError(baseRes);
  }
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
  fbs.Base.startBase(builder);
  fbs.Base.addMsg(builder, msg);
  fbs.Base.addMsgType(builder, fbs.Any.MakeTempDir);
  builder.finish(fbs.Base.endBase(builder));
  const resBuf = libdeno.send(builder.asUint8Array());
  assert(resBuf != null);
  const bb = new flatbuffers.ByteBuffer(new Uint8Array(resBuf!));
  const baseRes = fbs.Base.getRootAsBase(bb);
  maybeThrowError(baseRes);
  assert(fbs.Any.MakeTempDirRes === baseRes.msgType());
  const res = new fbs.MakeTempDirRes();
  assert(baseRes.msg(res) != null);
  const path = res.path();
  assert(path != null);
  return path!;
}

export function readFileSync(filename: string): Uint8Array {
  /* Ideally we could write
  const res = send({
    command: fbs.Command.READ_FILE_SYNC,
    readFileSyncFilename: filename
  });
  return res.readFileSyncData;
  */
  const builder = new flatbuffers.Builder();
  const filename_ = builder.createString(filename);
  fbs.ReadFileSync.startReadFileSync(builder);
  fbs.ReadFileSync.addFilename(builder, filename_);
  const msg = fbs.ReadFileSync.endReadFileSync(builder);
  fbs.Base.startBase(builder);
  fbs.Base.addMsg(builder, msg);
  fbs.Base.addMsgType(builder, fbs.Any.ReadFileSync);
  builder.finish(fbs.Base.endBase(builder));
  const resBuf = libdeno.send(builder.asUint8Array());
  assert(resBuf != null);
  // TypeScript does not track `assert` from a CFA perspective, therefore not
  // null assertion `!`
  const bb = new flatbuffers.ByteBuffer(new Uint8Array(resBuf!));
  const baseRes = fbs.Base.getRootAsBase(bb);
  maybeThrowError(baseRes);
  assert(fbs.Any.ReadFileSyncRes === baseRes.msgType());
  const res = new fbs.ReadFileSyncRes();
  assert(baseRes.msg(res) != null);
  const dataArray = res.dataArray();
  assert(dataArray != null);
  // TypeScript cannot track assertion above, therefore not null assertion
  return new Uint8Array(dataArray!);
}

export class FileInfo {
  private _isFile: boolean;
  private _isSymlink: boolean;
  len: number;
  modified: number;
  accessed: number;
  // Creation time is not available on all platforms.
  created: number | null;

  /* @internal */
  constructor(private _msg: fbs.StatSyncRes) {
    const created = this._msg.created().toFloat64();

    this._isFile = this._msg.isFile();
    this._isSymlink = this._msg.isSymlink();
    this.len = this._msg.len().toFloat64();
    this.modified = this._msg.modified().toFloat64();
    this.accessed = this._msg.accessed().toFloat64();
    this.created = created ? created: null;
  }

  isFile() {
    return this._isFile;
  }

  isDirectory() {
    return !this._isFile && !this._isSymlink;
  }

  isSymlink() {
    return this._isSymlink;
  }
}

export function lStatSync(filename: string): FileInfo {
  return statSyncInner(filename, true);
}

export function statSync(filename: string): FileInfo {
  return statSyncInner(filename, false);
}

function statSyncInner(filename: string, lstat: boolean): FileInfo {
  /* Ideally we could write
  const res = send({
    command: fbs.Command.STAT_FILE_SYNC,
    StatFilename: filename,
    StatLStat: lstat,
  });
  return new FileInfo(res);
   */
  const builder = new flatbuffers.Builder();
  const filename_ = builder.createString(filename);
  fbs.StatSync.startStatSync(builder);
  fbs.StatSync.addFilename(builder, filename_);
  fbs.StatSync.addLstat(builder, lstat);
  const msg = fbs.StatSync.endStatSync(builder);
  fbs.Base.startBase(builder);
  fbs.Base.addMsg(builder, msg);
  fbs.Base.addMsgType(builder, fbs.Any.StatSync);
  builder.finish(fbs.Base.endBase(builder));
  const resBuf = libdeno.send(builder.asUint8Array());
  assert(resBuf != null);
  // TypeScript does not track `assert` from a CFA perspective, therefore not
  // null assertion `!`
  const bb = new flatbuffers.ByteBuffer(new Uint8Array(resBuf!));
  const baseRes = fbs.Base.getRootAsBase(bb);
  maybeThrowError(baseRes);
  assert(fbs.Any.StatSyncRes === baseRes.msgType());
  const res = new fbs.StatSyncRes();
  assert(baseRes.msg(res) != null);
  // TypeScript cannot track assertion above, therefore not null assertion
  return new FileInfo(baseRes.msg(res)!);
}

export function writeFileSync(
  filename: string,
  data: Uint8Array,
  perm = 0o666
): void {
  /* Ideally we could write:
  const res = send({
    command: fbs.Command.WRITE_FILE_SYNC,
    writeFileSyncFilename: filename,
    writeFileSyncData: data,
    writeFileSyncPerm: perm
  });
  */
  const builder = new flatbuffers.Builder();
  const filename_ = builder.createString(filename);
  const dataOffset = fbs.WriteFileSync.createDataVector(builder, data);
  fbs.WriteFileSync.startWriteFileSync(builder);
  fbs.WriteFileSync.addFilename(builder, filename_);
  fbs.WriteFileSync.addData(builder, dataOffset);
  fbs.WriteFileSync.addPerm(builder, perm);
  const msg = fbs.WriteFileSync.endWriteFileSync(builder);
  fbs.Base.startBase(builder);
  fbs.Base.addMsg(builder, msg);
  fbs.Base.addMsgType(builder, fbs.Any.WriteFileSync);
  builder.finish(fbs.Base.endBase(builder));
  const resBuf = libdeno.send(builder.asUint8Array());
  if (resBuf != null) {
    const bb = new flatbuffers.ByteBuffer(new Uint8Array(resBuf!));
    const baseRes = fbs.Base.getRootAsBase(bb);
    maybeThrowError(baseRes);
  }
}
