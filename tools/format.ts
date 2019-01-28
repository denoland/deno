#!/usr/bin/env deno --allow-run
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as deno from "deno";
import { join } from "../js/deps/https/deno.land/x/std/fs/path.ts";
import { findFiles } from "./util.ts";

const clangFormat = join("third_party", "depot_tools", "clang-format");
const gn = join("third_party", "depot_tools", "gn");
const prettier = join(
  "third_party",
  "node_modules",
  "prettier",
  "bin-prettier.js"
);
const yapf = join("third_party", "python_packages", "bin", "yapf");
const rustfmt = join("third_party", "rustfmt", deno.platform.os, "rustfmt");
const rustfmtConfig = join("tools", "rustfmt.toml");

const run = (...args: string[]) => {
  if (deno.platform.os === "win") {
    args = ["cmd.exe", "/c", ...args];
  }
  return deno.run({ args, stdout: "null", stderr: "piped" }).status();
};

(async () => {
  console.log("clang_format");
  await run(
    clangFormat,
    "-i",
    "-style",
    "Google",
    ...findFiles(["libdeno"], [".cc", ".h"])
  );

  console.log("gn format");
  for (const fn of [
    "BUILD.gn",
    ".gn",
    ...findFiles(["build_extra", "libdeno"], [".gn", ".gni"])
  ]) {
    await run(gn, "format", fn);
  }

  console.log("yapf");
  await run(
    "python",
    yapf,
    "-i",
    ...findFiles(["tools", "build_extra"], [".py"], {
      skip: [join("tools", "clang")]
    })
  );

  console.log("prettier");
  await run(
    "node",
    prettier,
    "--write",
    "--loglevel=error",
    "rollup.config.js",
    ...findFiles(["."], [".json", ".md"], { depth: 1 }),
    ...findFiles(
      [".github", "js", "tests", "tools", "website"],
      [".js", ".json", ".ts", ".md"],
      { skip: [join("tools", "clang"), join("js", "deps")] }
    )
  );

  console.log("rustfmt");
  await run(
    rustfmt,
    "--config-path",
    rustfmtConfig,
    "build.rs",
    ...findFiles(["src"], [".rs"])
  );
})();
