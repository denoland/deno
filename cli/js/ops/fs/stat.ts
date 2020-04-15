// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";
import { build } from "../../build.ts";

export interface FileInfo {
  size: number;
  modified: number | null;
  accessed: number | null;
  created: number | null;
  dev: number | null;
  ino: number | null;
  mode: number | null;
  nlink: number | null;
  uid: number | null;
  gid: number | null;
  rdev: number | null;
  blksize: number | null;
  blocks: number | null;
  isFile: boolean;
  isDirectory: boolean;
  isSymlink: boolean;
}

export interface StatResponse {
  isFile: boolean;
  isDirectory: boolean;
  isSymlink: boolean;
  size: number;
  modified: number;
  accessed: number;
  created: number;
  // Null for stat(), but exists for readdir().
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

// @internal
export function parseFileInfo(response: StatResponse): FileInfo {
  const isUnix = build.os === "mac" || build.os === "linux";
  return {
    isFile: response.isFile,
    isDirectory: response.isDirectory,
    isSymlink: response.isSymlink,
    size: response.size,
    modified: response.modified ? response.modified : null,
    accessed: response.accessed ? response.accessed : null,
    created: response.created ? response.created : null,
    // Only non-null if on Unix
    dev: isUnix ? response.dev : null,
    ino: isUnix ? response.ino : null,
    mode: isUnix ? response.mode : null,
    nlink: isUnix ? response.nlink : null,
    uid: isUnix ? response.uid : null,
    gid: isUnix ? response.gid : null,
    rdev: isUnix ? response.rdev : null,
    blksize: isUnix ? response.blksize : null,
    blocks: isUnix ? response.blocks : null,
  };
}

export async function lstat(path: string): Promise<FileInfo> {
  const res = (await sendAsync("op_stat", {
    path,
    lstat: true,
  })) as StatResponse;
  return parseFileInfo(res);
}

export function lstatSync(path: string): FileInfo {
  const res = sendSync("op_stat", {
    path,
    lstat: true,
  }) as StatResponse;
  return parseFileInfo(res);
}

export async function stat(path: string): Promise<FileInfo> {
  const res = (await sendAsync("op_stat", {
    path,
    lstat: false,
  })) as StatResponse;
  return parseFileInfo(res);
}

export function statSync(path: string): FileInfo {
  const res = sendSync("op_stat", {
    path,
    lstat: false,
  }) as StatResponse;
  return parseFileInfo(res);
}
