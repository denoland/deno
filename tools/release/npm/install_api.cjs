// @ts-check
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
/** @type {string | undefined} */
let cachedIsMusl = undefined;

module.exports = {
  runInstall() {
    const denoFileName = os.platform() === "win32" ? "deno.exe" : "deno";
    const targetExecutablePath = path.join(
      __dirname,
      denoFileName,
    );

    if (fs.existsSync(targetExecutablePath)) {
      return targetExecutablePath;
    }

    const target = getTarget();
    const sourcePackagePath = path.dirname(
      require.resolve("@deno/" + target + "/package.json"),
    );
    const sourceExecutablePath = path.join(sourcePackagePath, denoFileName);

    if (!fs.existsSync(sourceExecutablePath)) {
      throw new Error(
        "Could not find executable for @deno/" + target + " at " +
          sourceExecutablePath,
      );
    }

    try {
      if (process.env.DPRINT_SIMULATED_READONLY_FILE_SYSTEM === "1") {
        console.warn("Simulating readonly file system for testing.");
        throw new Error("Throwing for testing purposes.");
      }

      // in order to make things faster the next time we run and to allow the
      // deno vscode extension to easily pick this up, copy the executable
      // into the deno package folder
      hardLinkOrCopy(sourceExecutablePath, targetExecutablePath);
      if (os.platform() !== "win32") {
        // chomd +x
        chmodX(targetExecutablePath);
      }
      return targetExecutablePath;
    } catch (err) {
      // this may fail on readonly file systems... in this case, fall
      // back to using the resolved package path
      if (process.env.DENO_DEBUG === "1") {
        console.warn(
          "Failed to copy executable from " +
            sourceExecutablePath + " to " + targetExecutablePath +
            ". Using resolved package path instead.",
          err,
        );
      }
      // use the path found in the specific package
      try {
        chmodX(sourceExecutablePath);
      } catch (_err) {
        // ignore
      }
      return sourceExecutablePath;
    }
  },
};

/** @filePath {string} */
function chmodX(filePath) {
  const perms = fs.statSync(filePath).mode;
  fs.chmodSync(filePath, perms | 0o111);
}

function getTarget() {
  const platform = os.platform();
  if (platform === "linux") {
    return platform + "-" + getArch() + "-" + getLinuxFamily();
  } else {
    return platform + "-" + getArch();
  }
}

function getArch() {
  const arch = os.arch();
  if (arch !== "arm64" && arch !== "x64") {
    throw new Error(
      "Unsupported architecture " + os.arch() +
        ". Only x64 and aarch64 binaries are available.",
    );
  }
  return arch;
}

function getLinuxFamily() {
  if (getIsMusl()) {
    throw new Error(
      "Musl is not supported. It's one of our priorities. Please upvote this issue: https://github.com/denoland/deno/issues/3711",
    );
    // return "musl";
  }
  return "glibc";

  function getIsMusl() {
    // code adapted from https://github.com/lovell/detect-libc
    // Copyright Apache 2.0 license, the detect-libc maintainers
    if (cachedIsMusl == null) {
      cachedIsMusl = innerGet();
    }
    return cachedIsMusl;

    function innerGet() {
      try {
        if (os.platform() !== "linux") {
          return false;
        }
        return isProcessReportMusl() || isConfMusl();
      } catch (err) {
        // just in case
        console.warn("Error checking if musl.", err);
        return false;
      }
    }

    function isProcessReportMusl() {
      if (!process.report) {
        return false;
      }
      const rawReport = process.report.getReport();
      const report = typeof rawReport === "string"
        ? JSON.parse(rawReport)
        : rawReport;
      if (!report || !(report.sharedObjects instanceof Array)) {
        return false;
      }
      return report.sharedObjects.some((o) =>
        o.includes("libc.musl-") || o.includes("ld-musl-")
      );
    }

    function isConfMusl() {
      const output = getCommandOutput();
      const [_, ldd1] = output.split(/[\r\n]+/);
      return ldd1 && ldd1.includes("musl");
    }

    function getCommandOutput() {
      try {
        const command =
          "getconf GNU_LIBC_VERSION 2>&1 || true; ldd --version 2>&1 || true";
        return require("child_process").execSync(command, { encoding: "utf8" });
      } catch (_err) {
        return "";
      }
    }
  }
}

/**
 * @param sourcePath {string}
 * @param destinationPath {string}
 */
function hardLinkOrCopy(sourcePath, destinationPath) {
  try {
    fs.linkSync(sourcePath, destinationPath);
  } catch {
    atomicCopyFile(sourcePath, destinationPath);
  }
}

/**
 * @param sourcePath {string}
 * @param destinationPath {string}
 */
function atomicCopyFile(sourcePath, destinationPath) {
  const crypto = require("crypto");
  const rand = crypto.randomBytes(4).toString("hex");
  const tempFilePath = destinationPath + "." + rand;
  fs.copyFileSync(sourcePath, tempFilePath);
  try {
    fs.renameSync(tempFilePath, destinationPath);
  } catch (err) {
    // will maybe throw when another process had already done this
    // so just ignore and delete the created temporary file
    try {
      fs.unlinkSync(tempFilePath);
    } catch (_err2) {
      // ignore
    }
    throw err;
  }
}
