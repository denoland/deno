// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This program fails if ./tools/format.js changes any files.

const p = Deno.run({
  cmd: [Deno.execPath(), "run", "--unstable", "-A", "tools/format.js"],
});
await p.status();
p.close();

const git = Deno.run({
  cmd: ["git", "status", "-uno", "--porcelain", "--ignore-submodules"],
  stdout: "piped",
});

await git.status();
const rawOut = await Deno.readAll(git.stdout);
const out = new TextDecoder().decode();

git.stdout.close();
git.close();

if (out) {
  console.log("run tools/format.py");
  console.log(out);
  Deno.exit(1);
}
