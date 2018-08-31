// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { ModuleInfo } from "./types";
import { deno as fbs } from "gen/msg_generated";
import { assert } from "./util";
import * as util from "./util";
import { flatbuffers } from "flatbuffers";
import { send } from "./fbs_util";

export function exit(exitCode = 0): never {
  const builder = new flatbuffers.Builder();
  fbs.Exit.startExit(builder);
  fbs.Exit.addCode(builder, exitCode);
  const msg = fbs.Exit.endExit(builder);
  send(builder, fbs.Any.Exit, msg);
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
  const baseRes = send(builder, fbs.Any.CodeFetch, msg);
  assert(baseRes != null);
  assert(fbs.Any.CodeFetchRes === baseRes!.msgType());
  const codeFetchRes = new fbs.CodeFetchRes();
  assert(baseRes!.msg(codeFetchRes) != null);
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
  const baseRes = send(builder, fbs.Any.CodeCache, msg);
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
  const baseRes = send(builder, fbs.Any.MakeTempDir, msg);
  assert(baseRes != null);
  assert(fbs.Any.MakeTempDirRes === baseRes!.msgType());
  const res = new fbs.MakeTempDirRes();
  assert(baseRes!.msg(res) != null);
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
  const baseRes = send(builder, fbs.Any.ReadFileSync, msg);
  assert(baseRes != null);
  assert(fbs.Any.ReadFileSyncRes === baseRes!.msgType());
  const res = new fbs.ReadFileSyncRes();
  assert(baseRes!.msg(res) != null);
  const dataArray = res.dataArray();
  assert(dataArray != null);
  return new Uint8Array(dataArray!);
}

function createEnv(_msg: fbs.EnvironRes): { [index:string]: string } {
  const env: { [index:string]: string } = {};

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
  send(builder, fbs.Any.SetEnv, msg);
}

/**
 * Returns a snapshot of the environment variables at invocation. Mutating a
 * property in the object will set that variable in the environment for
 * the process. The environment object will only accept `string`s or `number`s
 * as values.
 *     import { env } from "deno";
 *     const env = deno.env();
 *     console.log(env.SHELL)
 *     env.TEST_VAR = "HELLO";
 *
 *     const newEnv = deno.env();
 *     console.log(env.TEST_VAR == newEnv.TEST_VAR);
 */
export function env(): { [index:string]: string } {
  /* Ideally we could write
  const res = send({
    command: fbs.Command.ENV,
  });
  */
  const builder = new flatbuffers.Builder();
  fbs.Environ.startEnviron(builder);
  const msg = fbs.Environ.endEnviron(builder);
  const baseRes = send(builder, fbs.Any.Environ, msg)!;
  assert(fbs.Any.EnvironRes === baseRes.msgType());
  const res = new fbs.EnvironRes();
  assert(baseRes.msg(res) != null);
  // TypeScript cannot track assertion above, therefore not null assertion
  return createEnv(res);
}

/**
 * A FileInfo describes a file and is returned by `stat`, `lstat`,
 * `statSync`, `lstatSync`.
 */
export class FileInfo {
  private _isFile: boolean;
  private _isSymlink: boolean;
  /** The size of the file, in bytes. */
  len: number;
  /**
   * The last modification time of the file. This corresponds to the `mtime`
   * field from `stat` on Unix and `ftLastWriteTime` on Windows. This may not
   * be available on all platforms.
   */
  modified: number | null;
  /**
   * The last access time of the file. This corresponds to the `atime`
   * field from `stat` on Unix and `ftLastAccessTime` on Windows. This may not
   * be available on all platforms.
   */
  accessed: number | null;
  /**
   * The last access time of the file. This corresponds to the `birthtime`
   * field from `stat` on Unix and `ftCreationTime` on Windows. This may not
   * be available on all platforms.
   */
  created: number | null;

  /* @internal */
  constructor(private _msg: fbs.StatSyncRes) {
    const modified = this._msg.modified().toFloat64();
    const accessed = this._msg.accessed().toFloat64();
    const created = this._msg.created().toFloat64();

    this._isFile = this._msg.isFile();
    this._isSymlink = this._msg.isSymlink();
    this.len = this._msg.len().toFloat64();
    this.modified = modified ? modified : null;
    this.accessed = accessed ? accessed : null;
    this.created = created ? created : null;
  }

  /**
   * Returns whether this is info for a regular file. This result is mutually
   * exclusive to `FileInfo.isDirectory` and `FileInfo.isSymlink`.
   */
  isFile() {
    return this._isFile;
  }

  /**
   * Returns whether this is info for a regular directory. This result is
   * mutually exclusive to `FileInfo.isFile` and `FileInfo.isSymlink`.
   */
  isDirectory() {
    return !this._isFile && !this._isSymlink;
  }

  /**
   * Returns whether this is info for a symlink. This result is
   * mutually exclusive to `FileInfo.isFile` and `FileInfo.isDirectory`.
   */
  isSymlink() {
    return this._isSymlink;
  }
}

/**
 * Queries the file system for information on the path provided.
 * If the given path is a symlink information about the symlink will
 * be returned.
 * @returns FileInfo
 */
export function lStatSync(filename: string): FileInfo {
  return statSyncInner(filename, true);
}

/**
 * Queries the file system for information on the path provided.
 * `statSync` Will always follow symlinks.
 * @returns FileInfo
 */
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
  const baseRes = send(builder, fbs.Any.StatSync, msg);
  assert(baseRes != null);
  assert(fbs.Any.StatSyncRes === baseRes!.msgType());
  const res = new fbs.StatSyncRes();
  assert(baseRes!.msg(res) != null);
  return new FileInfo(res);
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
  send(builder, fbs.Any.WriteFileSync, msg);
}
