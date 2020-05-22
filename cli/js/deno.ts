// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Public deno module.
export {
  Buffer,
  readAll,
  readAllSync,
  writeAll,
  writeAllSync,
} from "./buffer.ts";
export { build } from "./build.ts";
export { chmodSync, chmod } from "./ops/fs/chmod.ts";
export { chownSync, chown } from "./ops/fs/chown.ts";
export { customInspect, inspect } from "./web/console.ts";
export { copyFileSync, copyFile } from "./ops/fs/copy_file.ts";
export { chdir, cwd } from "./ops/fs/dir.ts";
export { errors } from "./errors.ts";
export {
  File,
  open,
  openSync,
  create,
  createSync,
  stdin,
  stdout,
  stderr,
  seek,
  seekSync,
  OpenOptions,
} from "./files.ts";
export { read, readSync, write, writeSync } from "./ops/io.ts";
export { FsEvent, watchFs } from "./ops/fs_events.ts";
export { internalSymbol as internal } from "./internals.ts";
export {
  copy,
  iter,
  iterSync,
  SeekMode,
  Reader,
  ReaderSync,
  Writer,
  WriterSync,
  Closer,
  Seeker,
} from "./io.ts";
export {
  makeTempDirSync,
  makeTempDir,
  makeTempFileSync,
  makeTempFile,
  MakeTempOptions,
} from "./ops/fs/make_temp.ts";
export { metrics, Metrics } from "./ops/runtime.ts";
export { mkdirSync, mkdir, MkdirOptions } from "./ops/fs/mkdir.ts";
export { connect, listen, Listener, Conn } from "./net.ts";
export { env, exit, execPath } from "./ops/os.ts";
export { run, RunOptions, Process, ProcessStatus } from "./process.ts";
export { DirEntry, readDirSync, readDir } from "./ops/fs/read_dir.ts";
export { readFileSync, readFile } from "./read_file.ts";
export { readTextFileSync, readTextFile } from "./read_text_file.ts";
export { readLinkSync, readLink } from "./ops/fs/read_link.ts";
export { realPathSync, realPath } from "./ops/fs/real_path.ts";
export { removeSync, remove, RemoveOptions } from "./ops/fs/remove.ts";
export { renameSync, rename } from "./ops/fs/rename.ts";
export { resources, close } from "./ops/resources.ts";
export { FileInfo, statSync, lstatSync, stat, lstat } from "./ops/fs/stat.ts";
export { connectTls, listenTls } from "./tls.ts";
export { truncateSync, truncate } from "./ops/fs/truncate.ts";
export { isatty } from "./ops/tty.ts";
export { version } from "./version.ts";
export { writeFileSync, writeFile, WriteFileOptions } from "./write_file.ts";
export { writeTextFileSync, writeTextFile } from "./write_text_file.ts";
export const args: string[] = [];
export { TestDefinition, test } from "./testing.ts";

// These are internal Deno APIs.  We are marking them as internal so they do not
// appear in the runtime type library.
export { core } from "./core.ts";

export let pid: number;

export let noColor: boolean;
