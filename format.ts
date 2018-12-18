#!/usr/bin/env deno --allow-run

import { exit, run } from "deno";

async function main() {
  const prettier = run({
    args: ["bash", "-c", "prettier --write *.ts **/*.ts"]
  });
  const s = await prettier.status();
  exit(s.code);
}

main();
