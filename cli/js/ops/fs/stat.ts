// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";
import { FileInfo, FileInfoImpl } from "../../file_info.ts";

export interface StatResponse {
  isFile: boolean;
  isSymlink: boolean;
  size: number;
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

export async function lstat(path: string): Promise<FileInfo> {
  const res = (await sendAsync("op_stat", {
    path,
    lstat: true,
  })) as StatResponse;
  return new FileInfoImpl(res);
}

export function lstatSync(path: string): FileInfo {
  const res = sendSync("op_stat", {
    path,
    lstat: true,
  }) as StatResponse;
  return new FileInfoImpl(res);
}

export async function stat(path: string): Promise<FileInfo> {
  const res = (await sendAsync("op_stat", {
    path,
    lstat: false,
  })) as StatResponse;
  return new FileInfoImpl(res);
}

export function statSync(path: string): FileInfo {
  const res = sendSync("op_stat", {
    path,
    lstat: false,
  }) as StatResponse;
  return new FileInfoImpl(res);
}
