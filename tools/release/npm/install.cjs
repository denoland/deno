// @ts-check
// Copyright 2018-2026 the Deno authors. MIT license.
"use strict";

const path = require("path");
const fs = require("fs");
const os = require("os");

const exePath = require("./install_api.cjs").runInstall();

// On non-Windows platforms, try to replace the npm bin symlink to point
// directly at the native binary, avoiding Node.js startup overhead.
if (os.platform() !== "win32") {
  try {
    const binPath = path.join(__dirname, "..", ".bin", "deno");
    fs.lstatSync(binPath);
    const relative = path.relative(path.dirname(binPath), exePath);
    fs.unlinkSync(binPath);
    fs.symlinkSync(relative, binPath);
  } catch {
    // ignore - falls back to bin.cjs
  }
}
