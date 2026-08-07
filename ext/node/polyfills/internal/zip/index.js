// deno-lint-ignore-file
// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// This file is a port of Node.js `lib/internal/zip.js` (nodejs/node#64339).

(function () {
const { core, primordials } = __bootstrap;

// Public entry point for ZIP archive support in `node:zlib`. The
// implementation is split across `internal/zip/`:
//
//   constants        shared signatures, flags, symbols, and small values
//   binary           bounds-checked reads and buffer coercion
//   content-size     the module-global in-memory decompression ceiling
//   dos              MS-DOS date/time and CP437 legacy name/text decoding
//   extra-fields     TLV extra-field parsing and building
//   headers          reader-side header structures and archive-end location
//   header-builders  writer-side header/record builders
//   compression      deflate/inflate/zstd plumbing and member decoding
//   fs-util          fd read/write helpers
//   entry            ZipEntry
//   archive          createZipArchive()/zipFiles() serialization
//   buffer           ZipBuffer
//   file             ZipFile
//
// This barrel re-exports only the surface `lib/zlib.js` consumes.

const { ZipEntry } = core.loadExtScript("ext:deno_node/internal/zip/entry.js");
const { ZipBuffer } = core.loadExtScript(
  "ext:deno_node/internal/zip/buffer.js",
);
const { ZipFile } = core.loadExtScript("ext:deno_node/internal/zip/file.js");
const {
  createZipArchive,
  createZipArchiveSync,
  zipFiles,
} = core.loadExtScript("ext:deno_node/internal/zip/archive.js");
const {
  getMaxZipContentSize,
  setMaxZipContentSize,
} = core.loadExtScript("ext:deno_node/internal/zip/content-size.js");

return {
  ZipEntry,
  ZipFile,
  ZipBuffer,
  createZipArchive,
  createZipArchiveSync,
  zipFiles,
  getMaxZipContentSize,
  setMaxZipContentSize,
};
})();
