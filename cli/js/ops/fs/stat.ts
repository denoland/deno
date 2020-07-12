// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync, sendAsync } from "../dispatch_json.ts";
import { build } from "../../build.ts";
import { pathFromURL } from "../../util.ts";

export interface FileInfo {
  size: number;
  mtime: Date | null;
  atime: Date | null;
  birthtime: Date | null;
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
  mtime: number | null;
  atime: number | null;
  birthtime: number | null;
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
  const unix = build.os === "darwin" || build.os === "linux";
  return {
    isFile: response.isFile,
    isDirectory: response.isDirectory,
    isSymlink: response.isSymlink,
    size: response.size,
    mtime: response.mtime != null ? new Date(response.mtime) : null,
    atime: response.atime != null ? new Date(response.atime) : null,
    birthtime: response.birthtime != null ? new Date(response.birthtime) : null,
    // Only non-null if on Unix
    dev: unix ? response.dev : null,
    ino: unix ? response.ino : null,
    mode: unix ? response.mode : null,
    nlink: unix ? response.nlink : null,
    uid: unix ? response.uid : null,
    gid: unix ? response.gid : null,
    rdev: unix ? response.rdev : null,
    blksize: unix ? response.blksize : null,
    blocks: unix ? response.blocks : null,
  };
}

export function fstatSync(rid: number): FileInfo {
  return parseFileInfo(sendSync("op_fstat", { rid }));
}

export async function fstat(rid: number): Promise<FileInfo> {
  return parseFileInfo(await sendAsync("op_fstat", { rid }));
}

export async function lstat(path: string | URL): Promise<FileInfo> {
  const res = await sendAsync("op_stat", {
    path: pathFromURL(path),
    lstat: true,
  });
  return parseFileInfo(res);
}

export function lstatSync(path: string | URL): FileInfo {
  const res = sendSync("op_stat", {
    path: pathFromURL(path),
    lstat: true,
  });
  return parseFileInfo(res);
}

export async function stat(path: string | URL): Promise<FileInfo> {
  const res = await sendAsync("op_stat", {
    path: pathFromURL(path),
    lstat: false,
  });
  return parseFileInfo(res);
}

export function statSync(path: string | URL): FileInfo {
  const res = sendSync("op_stat", {
    path: pathFromURL(path),
    lstat: false,
  });
  return parseFileInfo(res);
}
