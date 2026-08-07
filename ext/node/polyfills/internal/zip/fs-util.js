// deno-lint-ignore-file
// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// This file is a port of Node.js `lib/internal/zip/fs-util.js` (nodejs/node#64339).

(function () {
const { core, primordials } = __bootstrap;

// Promise wrappers over the callback `fs` primitives (`ZipFile` works on a
// raw fd so one instance can serve both async and `Sync` methods), plus the
// short-read/short-write loops that guarantee a full transfer.

const {
  Promise,
} = primordials;

const {
  codes: {
    ERR_INVALID_STATE,
    ERR_ZIP_INVALID_ARCHIVE,
  },
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const lazyFs = core.createLazyLoader("node:fs");

// Promisified adapters over the callback `fs` primitives; individually trivial,
// they only exist so the async archive paths can `await` plain fd operations.
//
// `ZipFile` operates on a plain numeric file descriptor (rather than an
// `fs.promises` `FileHandle`) so that a single instance can support both the
// async and the `Sync` methods: `fs.read`/`fs.write`/`fs.fstat`/
// `fs.ftruncate`/`fs.close` all accept a raw fd directly, same as their
// `*Sync` counterparts, so both call sites share one open file underneath.
function fsOpenAsync(path, flag) {
  return new Promise((resolve, reject) => {
    lazyFs().open(path, flag, (err, fd) => (err ? reject(err) : resolve(fd)));
  });
}

function fsStatAsync(path) {
  return new Promise((resolve, reject) => {
    lazyFs().stat(path, (err, stats) => (err ? reject(err) : resolve(stats)));
  });
}

function fsLstatAsync(path) {
  return new Promise((resolve, reject) => {
    lazyFs().lstat(path, (err, stats) => (err ? reject(err) : resolve(stats)));
  });
}

function fsReadlinkAsync(path) {
  return new Promise((resolve, reject) => {
    lazyFs().readlink(
      path,
      "utf8",
      (err, target) => (err ? reject(err) : resolve(target)),
    );
  });
}

function fsCloseAsync(fd) {
  return new Promise((resolve, reject) => {
    lazyFs().close(fd, (err) => (err ? reject(err) : resolve()));
  });
}

function fsFstatAsync(fd) {
  return new Promise((resolve, reject) => {
    lazyFs().fstat(fd, (err, stats) => (err ? reject(err) : resolve(stats)));
  });
}

function fsReadAsync(fd, buffer, offset, length, position) {
  return new Promise((resolve, reject) => {
    lazyFs().read(
      fd,
      buffer,
      offset,
      length,
      position,
      (err, bytesRead) => (err ? reject(err) : resolve(bytesRead)),
    );
  });
}

function fsWriteAsync(fd, buffer, offset, length, position) {
  return new Promise((resolve, reject) => {
    lazyFs().write(
      fd,
      buffer,
      offset,
      length,
      position,
      (err, bytesWritten) => (err ? reject(err) : resolve(bytesWritten)),
    );
  });
}

function fsFtruncateAsync(fd, len) {
  return new Promise((resolve, reject) => {
    lazyFs().ftruncate(fd, len, (err) => (err ? reject(err) : resolve()));
  });
}

// `read(2)` may return fewer bytes than requested without hitting EOF, so loop
// until `buffer` is entirely filled; a genuine short read (0 bytes) means the
// archive is truncated where a full header/record was expected.
async function readFdFully(fd, buffer, position) {
  let done = 0;
  while (done < buffer.length) {
    const bytesRead = await fsReadAsync(
      fd,
      buffer,
      done,
      buffer.length - done,
      position + done,
    );
    if (bytesRead <= 0) {
      throw new ERR_ZIP_INVALID_ARCHIVE("unexpected end of file");
    }
    done += bytesRead;
  }
}

// Synchronous counterpart of `readFdFully()`; same short-read loop.
function readFdFullySync(fd, buffer, position) {
  let done = 0;
  while (done < buffer.length) {
    const bytesRead = lazyFs().readSync(
      fd,
      buffer,
      done,
      buffer.length - done,
      position + done,
    );
    if (bytesRead <= 0) {
      throw new ERR_ZIP_INVALID_ARCHIVE("unexpected end of file");
    }
    done += bytesRead;
  }
}

// `write(2)` may write fewer bytes than asked - most notably when an error
// (ENOSPC, EIO, a full NFS commit) strikes after partial progress, which
// surfaces as a short, error-free count. Advancing archive offsets by the
// intended length would then silently corrupt the file, so every archive
// write loops until the buffer is fully on disk; retrying the remainder
// re-encounters and surfaces the underlying error.
async function writeFdFully(fd, buffer, position) {
  let done = 0;
  while (done < buffer.length) {
    const bytesWritten = await fsWriteAsync(
      fd,
      buffer,
      done,
      buffer.length - done,
      position + done,
    );
    if (bytesWritten <= 0) {
      throw new ERR_INVALID_STATE("a write to the archive made no progress");
    }
    done += bytesWritten;
  }
}

// Synchronous counterpart of `writeFdFully()`; same short-write loop.
function writeFdFullySync(fd, buffer, position) {
  let done = 0;
  while (done < buffer.length) {
    const bytesWritten = lazyFs().writeSync(
      fd,
      buffer,
      done,
      buffer.length - done,
      position + done,
    );
    if (bytesWritten <= 0) {
      throw new ERR_INVALID_STATE("a write to the archive made no progress");
    }
    done += bytesWritten;
  }
}

return {
  fsOpenAsync,
  fsStatAsync,
  fsLstatAsync,
  fsReadlinkAsync,
  fsCloseAsync,
  fsFstatAsync,
  fsFtruncateAsync,
  readFdFully,
  readFdFullySync,
  writeFdFully,
  writeFdFullySync,
};
})();
