// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core } = globalThis.__bootstrap;
const lazyFs = core.createLazyLoader("node:fs");

const fsPromises = lazyFs().promises;

return {
  access: fsPromises.access,
  constants: fsPromises.constants,
  copyFile: fsPromises.copyFile,
  open: fsPromises.open,
  opendir: fsPromises.opendir,
  rename: fsPromises.rename,
  truncate: fsPromises.truncate,
  rm: fsPromises.rm,
  rmdir: fsPromises.rmdir,
  mkdir: fsPromises.mkdir,
  readdir: fsPromises.readdir,
  readlink: fsPromises.readlink,
  symlink: fsPromises.symlink,
  lstat: fsPromises.lstat,
  stat: fsPromises.stat,
  statfs: fsPromises.statfs,
  link: fsPromises.link,
  unlink: fsPromises.unlink,
  chmod: fsPromises.chmod,
  lchmod: fsPromises.lchmod,
  lchown: fsPromises.lchown,
  chown: fsPromises.chown,
  utimes: fsPromises.utimes,
  lutimes: fsPromises.lutimes,
  realpath: fsPromises.realpath,
  mkdtemp: fsPromises.mkdtemp,
  mkdtempDisposable: fsPromises.mkdtempDisposable,
  writeFile: fsPromises.writeFile,
  appendFile: fsPromises.appendFile,
  readFile: fsPromises.readFile,
  watch: fsPromises.watch,
  cp: fsPromises.cp,
  glob: fsPromises.glob,
  fsPromises,
};
})();
