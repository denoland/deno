#!/usr/bin/env deno --allow-run
// Copyright 2018 the Deno authors. All rights reserved. MIT license.

import { readAll, exit, run } from "deno";

async function checkVersion() {
  const prettierVersion = run({
    args: ["bash", "-c", "prettier --version"],
    stdout: "piped"
  });
  const b = await readAll(prettierVersion.stdout);
  const s = await prettierVersion.status();
  if (s.code != 0) {
    console.log("error calling prettier --version error");
    exit(s.code);
  }
  const version = new TextDecoder().decode(b).trim();
  const requiredVersion = "1.15";
  if (!version.startsWith(requiredVersion)) {
    console.log(`Required prettier version: ${requiredVersion}`);
    console.log(`Installed prettier version: ${version}`);
    exit(1);
  }
}

async function main() {
  await checkVersion();

  const prettier = run({
    args: ["bash", "-c", "prettier --write *.ts */**/*.ts *.md */**/*.md"]
  });
  const s = await prettier.status();
  exit(s.code);
}

main();
