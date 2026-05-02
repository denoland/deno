// @ts-check
// Copyright 2018-2026 the Deno authors. MIT license.
"use strict";

const api = require("./install_api.cjs");
const exePath = api.runInstall();
try {
  api.replaceBinEntry(exePath);
} catch (_err) {
  // ignore - falls back to bin.cjs
}
