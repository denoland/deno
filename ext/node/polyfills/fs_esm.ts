// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/fs.ts");

// Export all simple bindings first. `internal/fs/{handle,promises,streams}.ts`
// access `lazyFs().<method>` at top-level when they're loaded, so every method
// they touch must have its `export const` evaluated before we trigger their
// load (via `ReadStream`, `WriteStream`, `createReadStream`, `createWriteStream`,
// and `promises` below).
export const _toUnixTimestamp = mod._toUnixTimestamp;
export const access = mod.access;
export const accessSync = mod.accessSync;
export const appendFile = mod.appendFile;
export const appendFileSync = mod.appendFileSync;
export const BigIntStats = mod.BigIntStats;
export const CFISBIS = mod.CFISBIS;
export const chmod = mod.chmod;
export const chmodSync = mod.chmodSync;
export const chown = mod.chown;
export const chownSync = mod.chownSync;
export const close = mod.close;
export const closeSync = mod.closeSync;
export const constants = mod.constants;
export const convertFileInfoToBigIntStats = mod.convertFileInfoToBigIntStats;
export const convertFileInfoToStats = mod.convertFileInfoToStats;
export const copyFile = mod.copyFile;
export const copyFileSync = mod.copyFileSync;
export const cp = mod.cp;
export const cpSync = mod.cpSync;
export const Dir = mod.Dir;
export const Dirent = mod.Dirent;
export const exists = mod.exists;
export const existsSync = mod.existsSync;
export const fchmod = mod.fchmod;
export const fchmodSync = mod.fchmodSync;
export const fchown = mod.fchown;
export const fchownSync = mod.fchownSync;
export const fdatasync = mod.fdatasync;
export const fdatasyncSync = mod.fdatasyncSync;
export const fstat = mod.fstat;
export const fstatSync = mod.fstatSync;
export const fsync = mod.fsync;
export const fsyncSync = mod.fsyncSync;
export const ftruncate = mod.ftruncate;
export const ftruncateSync = mod.ftruncateSync;
export const futimes = mod.futimes;
export const futimesSync = mod.futimesSync;
export const glob = mod.glob;
export const globSync = mod.globSync;
export const lchmod = mod.lchmod;
export const lchmodSync = mod.lchmodSync;
export const lchown = mod.lchown;
export const lchownSync = mod.lchownSync;
export const link = mod.link;
export const linkSync = mod.linkSync;
export const lstat = mod.lstat;
export const lstatSync = mod.lstatSync;
export const lutimes = mod.lutimes;
export const lutimesSync = mod.lutimesSync;
export const mkdir = mod.mkdir;
export const mkdirSync = mod.mkdirSync;
export const mkdtemp = mod.mkdtemp;
export const mkdtempDisposableSync = mod.mkdtempDisposableSync;
export const mkdtempSync = mod.mkdtempSync;
export const open = mod.open;
export const openAsBlob = mod.openAsBlob;
export const opendir = mod.opendir;
export const opendirSync = mod.opendirSync;
export const openSync = mod.openSync;
export const read = mod.read;
export const readdir = mod.readdir;
export const readdirSync = mod.readdirSync;
export const readFile = mod.readFile;
export const readFilePromise = mod.readFilePromise;
export const readFileSync = mod.readFileSync;
export const readlink = mod.readlink;
export const readlinkPromise = mod.readlinkPromise;
export const readlinkSync = mod.readlinkSync;
export const readSync = mod.readSync;
export const readv = mod.readv;
export const readvPromise = mod.readvPromise;
export const readvSync = mod.readvSync;
export const realpath = mod.realpath;
export const realpathSync = mod.realpathSync;
export const rename = mod.rename;
export const renameSync = mod.renameSync;
export const rm = mod.rm;
export const rmdir = mod.rmdir;
export const rmdirSync = mod.rmdirSync;
export const rmSync = mod.rmSync;
export const stat = mod.stat;
export const Stats = mod.Stats;
export const statfs = mod.statfs;
export const statfsSync = mod.statfsSync;
export const statSync = mod.statSync;
export const symlink = mod.symlink;
export const symlinkSync = mod.symlinkSync;
export const SyncWriteStream = mod.SyncWriteStream;
export const truncate = mod.truncate;
export const truncateSync = mod.truncateSync;
export const unlink = mod.unlink;
export const unlinkSync = mod.unlinkSync;
export const unwatchFile = mod.unwatchFile;
export const Utf8Stream = mod.Utf8Stream;
export const utimes = mod.utimes;
export const utimesSync = mod.utimesSync;
export const watch = mod.watch;
export const watchFile = mod.watchFile;
export const watchPromise = mod.watchPromise;
export const write = mod.write;
export const writeFile = mod.writeFile;
export const writeFileSync = mod.writeFileSync;
export const writeSync = mod.writeSync;
export const writev = mod.writev;
export const writevSync = mod.writevSync;

// These trigger loading of internal/fs/{streams,handle,promises}, which in
// turn read methods off the `node:fs` namespace via `lazyFs()` at their own
// top level. Keep them after every other export so those reads find a fully
// initialized namespace instead of hitting TDZ.
export const createReadStream = mod.createReadStream;
export const createWriteStream = mod.createWriteStream;
export const ReadStream = mod.ReadStream;
export const WriteStream = mod.WriteStream;
export const promises = mod.promises;

export default mod;
