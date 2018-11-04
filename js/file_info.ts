// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";

/** A FileInfo describes a file and is returned by `stat`, `lstat`,
 * `statSync`, `lstatSync`.
 */
export interface FileInfo {
  /** The size of the file, in bytes. */
  len: number;
  /** The last modification time of the file. This corresponds to the `mtime`
   * field from `stat` on Unix and `ftLastWriteTime` on Windows. This may not
   * be available on all platforms.
   */
  modified: number | null;
  /** The last access time of the file. This corresponds to the `atime`
   * field from `stat` on Unix and `ftLastAccessTime` on Windows. This may not
   * be available on all platforms.
   */
  accessed: number | null;
  /** The last access time of the file. This corresponds to the `birthtime`
   * field from `stat` on Unix and `ftCreationTime` on Windows. This may not
   * be available on all platforms.
   */
  created: number | null;
  /** The underlying raw st_mode bits that contain the standard Unix permissions
   * for this file/directory. TODO Match behavior with Go on windows for mode.
   */
  mode: number | null;

  /** Returns the file or directory name. */
  name: string | null;

  /** Returns the file or directory path. */
  path: string | null;

  /** Returns whether this is info for a regular file. This result is mutually
   * exclusive to `FileInfo.isDirectory` and `FileInfo.isSymlink`.
   */
  isFile(): boolean;

  /** Returns whether this is info for a regular directory. This result is
   * mutually exclusive to `FileInfo.isFile` and `FileInfo.isSymlink`.
   */
  isDirectory(): boolean;

  /** Returns whether this is info for a symlink. This result is
   * mutually exclusive to `FileInfo.isFile` and `FileInfo.isDirectory`.
   */
  isSymlink(): boolean;
}

// @internal
export class FileInfoImpl implements FileInfo {
  private readonly _isFile: boolean;
  private readonly _isSymlink: boolean;
  len: number;
  modified: number | null;
  accessed: number | null;
  created: number | null;
  mode: number | null;
  name: string | null;
  path: string | null;

  /* @internal */
  constructor(private _inner: msg.StatRes) {
    const modified = this._inner.modified().toFloat64();
    const accessed = this._inner.accessed().toFloat64();
    const created = this._inner.created().toFloat64();
    const hasMode = this._inner.hasMode();
    const mode = this._inner.mode(); // negative for invalid mode (Windows)
    const name = this._inner.name();
    const path = this._inner.path();

    this._isFile = this._inner.isFile();
    this._isSymlink = this._inner.isSymlink();
    this.len = this._inner.len().toFloat64();
    this.modified = modified ? modified : null;
    this.accessed = accessed ? accessed : null;
    this.created = created ? created : null;
    // null on Windows
    this.mode = hasMode ? mode : null;
    this.name = name ? name : null;
    this.path = path ? path : null;
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
