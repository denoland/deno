#!/usr/bin/env node
// Copyright 2018-2025 the Deno authors. MIT license.

// @ts-check
const path = require("path");
const child_process = require("child_process");
const os = require("os");
const fs = require("fs");

const exePath = path.join(
  __dirname,
  os.platform() === "win32" ? "deno.exe" : "deno",
);

if (!fs.existsSync(exePath)) {
  try {
    const resolvedExePath = require("./install_api.cjs").runInstall();
    runDenoExe(resolvedExePath);
  } catch (err) {
    if (err !== undefined && typeof err.message === "string") {
      console.error(err.message);
    } else {
      console.error(err);
    }
    process.exit(1);
  }
} else {
  runDenoExe(exePath);
}

/** @param exePath {string} */
function runDenoExe(exePath) {
  const result = child_process.spawnSync(
    exePath,
    process.argv.slice(2),
    { stdio: "inherit" },
  );
  if (result.error) {
    throw result.error;
  }

  throwIfNoExePath();

  process.exitCode = result.status;

  function throwIfNoExePath() {
    if (!fs.existsSync(exePath)) {
      throw new Error(
        "Could not find exe at path '" + exePath +
          "'. Maybe try running deno again.",
      );
    }
  }
}
