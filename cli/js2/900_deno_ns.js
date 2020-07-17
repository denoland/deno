// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module exports stable Deno APIs.

((window) => {
  // export { watchFs } from "./ops/fs_events.ts";
  // export { Process, run } from "./process.ts";
  // export { readFileSync, readFile } from "./read_file.ts";
  // export { readTextFileSync, readTextFile } from "./read_text_file.ts";
  // export { isatty } from "./ops/tty.ts";
  // export { writeFileSync, writeFile } from "./write_file.ts";
  // export { writeTextFileSync, writeTextFile } from "./write_text_file.ts";
  // export { test } from "./testing.ts";

  window.Deno = {
    ...window.Deno,
    ...{
      chmodSync: window.__fs.chmodSync,
      chmod: window.__fs.chmod,
      chown: window.__fs.chown,
      chownSync: window.__fs.chownSync,
      copyFileSync: window.__fs.copyFileSync,
      cwd: window.__fs.cwd,
      makeTempDirSync: window.__fs.makeTempDirSync,
      makeTempDir: window.__fs.makeTempDir,
      makeTempFileSync: window.__fs.makeTempFileSync,
      makeTempFile: window.__fs.makeTempFile,
      mkdirSync: window.__fs.mkdirSync,
      mkdir: window.__fs.mkdir,
      chdir: window.__fs.chdir,
      copyFile: window.__fs.copyFile,
      readDirSync: window.__fs.readDirSync,
      readDir: window.__fs.readDir,
      readLinkSync: window.__fs.readLinkSync,
      readLink: window.__fs.readLink,
      realPathSync: window.__fs.realPathSync,
      realPath: window.__fs.realPath,
      removeSync: window.__fs.removeSync,
      remove: window.__fs.remove,
      renameSync: window.__fs.renameSync,
      rename: window.__fs.rename,
      version: window.__version.version,
      build: window.__build.build,
      statSync: window.__fs.statSync,
      lstatSync: window.__fs.lstatSync,
      stat: window.__fs.stat,
      lstat: window.__fs.lstat,
      truncateSync: window.__fs.truncateSync,
      truncate: window.__fs.truncate,
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
