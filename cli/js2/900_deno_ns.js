// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module exports stable Deno APIs.

((window) => {
  // export {
  //   Buffer,
  //   readAll,
  //   readAllSync,
  //   writeAll,
  //   writeAllSync,
  // } from "./buffer.ts";
  // export { chmodSync, chmod } from "./ops/fs/chmod.ts";
  // export { chownSync, chown } from "./ops/fs/chown.ts";
  // export { copyFileSync, copyFile } from "./ops/fs/copy_file.ts";
  // export { chdir, cwd } from "./ops/fs/dir.ts";
  // export {
  //   File,
  //   open,
  //   openSync,
  //   create,
  //   createSync,
  //   stdin,
  //   stdout,
  //   stderr,
  //   seek,
  //   seekSync,
  // } from "./files.ts";
  // export { read, readSync, write, writeSync } from "./ops/io.ts";
  // export { watchFs } from "./ops/fs_events.ts";
  // export { copy, iter, iterSync } from "./io.ts";
  // export { SeekMode } from "./io.ts";
  //   Reader,
  //   ReaderSync,
  //   Writer,
  //   WriterSync,
  //   Closer,
  //   Seeker,
  // } from "./io.ts";
  // export {
  //   makeTempDirSync,
  //   makeTempDir,
  //   makeTempFileSync,
  //   makeTempFile,
  // } from "./ops/fs/make_temp.ts";
  // export { mkdirSync, mkdir } from "./ops/fs/mkdir.ts";
  // export { connect, listen } from "./net.ts";
  // export { Process, run } from "./process.ts";
  // export { readDirSync, readDir } from "./ops/fs/read_dir.ts";
  // export { readFileSync, readFile } from "./read_file.ts";
  // export { readTextFileSync, readTextFile } from "./read_text_file.ts";
  // export { readLinkSync, readLink } from "./ops/fs/read_link.ts";
  // export { realPathSync, realPath } from "./ops/fs/real_path.ts";
  // export { removeSync, remove } from "./ops/fs/remove.ts";
  // export { renameSync, rename } from "./ops/fs/rename.ts";
  // export { statSync, lstatSync, stat, lstat } from "./ops/fs/stat.ts";
  // export { connectTls, listenTls } from "./tls.ts";
  // export { truncateSync, truncate } from "./ops/fs/truncate.ts";
  // export { isatty } from "./ops/tty.ts";
  // export { writeFileSync, writeFile } from "./write_file.ts";
  // export { writeTextFileSync, writeTextFile } from "./write_text_file.ts";
  // export { test } from "./testing.ts";

  window.Deno = {
    ...window.Deno,
    ...{
      version: window.__version.version,
      build: window.__build.build,
      errors: window.__errors.errors,
      customInspect: window.__console.customInspect,
      inspect: window.__console.inspect,
      env: window.__os.env,
      exit: window.__os.exit,
      execPath: window.__os.execPath,
      resources: window.__resources.resources,
      close: window.__resources.close,
    },
  };
})(this);
