// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as fbs from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import * as dispatch from "./dispatch";
import { assert } from "./util";

/**
 * A FileInfo describes a file and is returned by `stat`, `lstat`,
 * `statSync`, `lstatSync`.
 */
export interface FileInfo {
  readonly _isFile: boolean;
  readonly _isSymlink: boolean;
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
  /**
   * The underlying raw st_mode bits that contain the standard Unix permissions
   * for this file/directory. TODO Match behavior with Go on windows for mode.
   */
  mode: number | null;

  /**
   * Returns whether this is info for a regular file. This result is mutually
   * exclusive to `FileInfo.isDirectory` and `FileInfo.isSymlink`.
   */
  isFile(): boolean;

  /**
   * Returns whether this is info for a regular directory. This result is
   * mutually exclusive to `FileInfo.isFile` and `FileInfo.isSymlink`.
   */
  isDirectory(): boolean;

  /**
   * Returns whether this is info for a symlink. This result is
   * mutually exclusive to `FileInfo.isFile` and `FileInfo.isDirectory`.
   */
  isSymlink(): boolean;
}

class FileInfoImpl implements FileInfo {
  readonly _isFile: boolean;
  readonly _isSymlink: boolean;
  len: number;
  modified: number | null;
  accessed: number | null;
  created: number | null;
  mode: number | null;

  /* @internal */
  constructor(private _msg: fbs.StatRes) {
    const modified = this._msg.modified().toFloat64();
    const accessed = this._msg.accessed().toFloat64();
    const created = this._msg.created().toFloat64();
    const hasMode = this._msg.hasMode();
    const mode = this._msg.mode(); // negative for invalid mode (Windows)

    this._isFile = this._msg.isFile();
    this._isSymlink = this._msg.isSymlink();
    this.len = this._msg.len().toFloat64();
    this.modified = modified ? modified : null;
    this.accessed = accessed ? accessed : null;
    this.created = created ? created : null;
    // null on Windows
    this.mode = hasMode ? mode : null;
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

/**
 * Queries the file system for information on the path provided.
 * If the given path is a symlink information about the symlink will
 * be returned.
 *
 *     import { lstat } from "deno";
 *     const fileInfo = await lstat("hello.txt");
 *     assert(fileInfo.isFile());
 */
export async function lstat(filename: string): Promise<FileInfo> {
  return res(await dispatch.sendAsync(...req(filename, true)));
}

/**
 * Queries the file system for information on the path provided synchronously.
 * If the given path is a symlink information about the symlink will
 * be returned.
 *
 *     import { lstatSync } from "deno";
 *     const fileInfo = lstatSync("hello.txt");
 *     assert(fileInfo.isFile());
 */
export function lstatSync(filename: string): FileInfo {
  return res(dispatch.sendSync(...req(filename, true)));
}

/**
 * Queries the file system for information on the path provided.
 * `stat` Will always follow symlinks.
 *
 *     import { stat } from "deno";
 *     const fileInfo = await stat("hello.txt");
 *     assert(fileInfo.isFile());
 */
export async function stat(filename: string): Promise<FileInfo> {
  return res(await dispatch.sendAsync(...req(filename, false)));
}

/**
 * Queries the file system for information on the path provided synchronously.
 * `statSync` Will always follow symlinks.
 *
 *     import { statSync } from "deno";
 *     const fileInfo = statSync("hello.txt");
 *     assert(fileInfo.isFile());
 */
export function statSync(filename: string): FileInfo {
  return res(dispatch.sendSync(...req(filename, false)));
}

function req(
  filename: string,
  lstat: boolean
): [flatbuffers.Builder, fbs.Any, flatbuffers.Offset] {
  const builder = new flatbuffers.Builder();
  const filename_ = builder.createString(filename);
  fbs.Stat.startStat(builder);
  fbs.Stat.addFilename(builder, filename_);
  fbs.Stat.addLstat(builder, lstat);
  const msg = fbs.Stat.endStat(builder);
  return [builder, fbs.Any.Stat, msg];
}

function res(baseRes: null | fbs.Base): FileInfo {
  assert(baseRes != null);
  assert(fbs.Any.StatRes === baseRes!.msgType());
  const res = new fbs.StatRes();
  assert(baseRes!.msg(res) != null);
  return new FileInfoImpl(res);
}
