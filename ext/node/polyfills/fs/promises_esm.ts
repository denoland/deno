// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/fs/promises.ts");

export const access = mod.access;
export const constants = mod.constants;
export const copyFile = mod.copyFile;
export const open = mod.open;
export const opendir = mod.opendir;
export const rename = mod.rename;
export const truncate = mod.truncate;
export const rm = mod.rm;
export const rmdir = mod.rmdir;
export const mkdir = mod.mkdir;
export const readdir = mod.readdir;
export const readlink = mod.readlink;
export const symlink = mod.symlink;
export const lstat = mod.lstat;
export const stat = mod.stat;
export const statfs = mod.statfs;
export const link = mod.link;
export const unlink = mod.unlink;
export const chmod = mod.chmod;
export const lchmod = mod.lchmod;
export const lchown = mod.lchown;
export const chown = mod.chown;
export const utimes = mod.utimes;
export const lutimes = mod.lutimes;
export const realpath = mod.realpath;
export const mkdtemp = mod.mkdtemp;
export const mkdtempDisposable = mod.mkdtempDisposable;
export const writeFile = mod.writeFile;
export const appendFile = mod.appendFile;
export const readFile = mod.readFile;
export const watch = mod.watch;
export const cp = mod.cp;
export const glob = mod.glob;

export default mod.fsPromises;
