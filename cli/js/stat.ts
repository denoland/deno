// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";
import { FileInfo, FileInfoImpl } from "./file_info.ts";

export interface StatResponse {
  isFile: boolean;
  isSymlink: boolean;
  len: number;
  modified: number;
  accessed: number;
  created: number;
  name: string | null;
  // Unix only members
  dev: number;
  ino: number;
  mode: number;
  nlink: number;
  uid: number;
  gid: number;
  rdev: number;
  blksize: number;
  blocks: number;
}

/** Queries the file system for information on the path provided. If the given
 * path is a symlink information about the symlink will be returned.
 *
 *       const fileInfo = await Deno.lstat("hello.txt");
 *       assert(fileInfo.isFile());
 */
export async function lstat(filename: string): Promise<FileInfo> {
  const res = (await sendAsync("op_stat", {
    filename,
    lstat: true
  })) as StatResponse;
  return new FileInfoImpl(res);
}

/** Queries the file system for information on the path provided synchronously.
 * If the given path is a symlink information about the symlink will be
 * returned.
 *
 *       const fileInfo = Deno.lstatSync("hello.txt");
 *       assert(fileInfo.isFile());
 */
export function lstatSync(filename: string): FileInfo {
  const res = sendSync("op_stat", {
    filename,
    lstat: true
  }) as StatResponse;
  return new FileInfoImpl(res);
}

/** Queries the file system for information on the path provided. `stat` Will
 * always follow symlinks.
 *
 *       const fileInfo = await Deno.stat("hello.txt");
 *       assert(fileInfo.isFile());
 */
export async function stat(filename: string): Promise<FileInfo> {
  const res = (await sendAsync("op_stat", {
    filename,
    lstat: false
  })) as StatResponse;
  return new FileInfoImpl(res);
}

/** Queries the file system for information on the path provided synchronously.
 * `statSync` Will always follow symlinks.
 *
 *       const fileInfo = Deno.statSync("hello.txt");
 *       assert(fileInfo.isFile());
 */
export function statSync(filename: string): FileInfo {
  const res = sendSync("op_stat", {
    filename,
    lstat: false
  }) as StatResponse;
  return new FileInfoImpl(res);
}
