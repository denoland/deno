// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module exports stable Deno APIs.

((window) => {
  // export { chmodSync, chmod } from "./ops/fs/chmod.ts";
  // export { chownSync, chown } from "./ops/fs/chown.ts";
  // export { copyFileSync, copyFile } from "./ops/fs/copy_file.ts";
  // export { chdir, cwd } from "./ops/fs/dir.ts";
  // export { watchFs } from "./ops/fs_events.ts";
  // export {
  //   makeTempDirSync,
  //   makeTempDir,
  //   makeTempFileSync,
  //   makeTempFile,
  // } from "./ops/fs/make_temp.ts";
  // export { mkdirSync, mkdir } from "./ops/fs/mkdir.ts";
  // export { Process, run } from "./process.ts";
  // export { readDirSync, readDir } from "./ops/fs/read_dir.ts";
  // export { readFileSync, readFile } from "./read_file.ts";
  // export { readTextFileSync, readTextFile } from "./read_text_file.ts";
  // export { readLinkSync, readLink } from "./ops/fs/read_link.ts";
  // export { realPathSync, realPath } from "./ops/fs/real_path.ts";
  // export { removeSync, remove } from "./ops/fs/remove.ts";
  // export { renameSync, rename } from "./ops/fs/rename.ts";
  // export { statSync, lstatSync, stat, lstat } from "./ops/fs/stat.ts";
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
      Buffer: window.__buffer.Buffer,
      readAll: window.__buffer.readAll,
      readAllSync: window.__buffer.readAllSync,
      writeAll: window.__buffer.writeAll,
      writeAllSync: window.__buffer.writeAllSync,
      copy: window.__io.copy,
      iter: window.__io.iter,
      iterSync: window.__io.iterSync,
      SeekMode: window.__io.SeekMode,
      read: window.__io.read,
      readSync: window.__io.readSync,
      write: window.__io.write,
      writeSync: window.__io.writeSync,
      File: window.__files.File,
      open: window.__files.open,
      openSync: window.__files.openSync,
      create: window.__files.create,
      createSync: window.__files.createSync,
      stdin: window.__files.stdin,
      stdout: window.__files.stdout,
      stderr: window.__files.stderr,
      seek: window.__files.seek,
      seekSync: window.__files.seekSync,
      connect: window.__net.connect,
      listen: window.__net.listen,
      connectTls: window.__tls.connectTls,
      listenTls: window.__tls.listenTls,
    },
  };
})(this);
