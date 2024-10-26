#!/usr/bin/env -S deno run --allow-env --allow-read --allow-write --allow-run=git,cargo
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import $ from "https://deno.land/x/dax@0.32.0/mod.ts";

if (Deno.args.length === 0) {
  $.log(
    "Usage: build_bench [-v] [--profile release|debug] commit1 [commit2 [comment3...]]",
  );
  Deno.exit(1);
}

const args = Deno.args.slice();
let verbose = false;
if (args[0] == "-v") {
  args.shift();
  verbose = true;
}

let profile = "release";
if (args[0] == "--profile") {
  args.shift();
  profile = args.shift();
}

function exit(msg: string) {
  $.logError(msg);
  Deno.exit(1);
}

// Make sure the .git dir exists
const gitDir = Deno.cwd() + "/.git";
await Deno.stat(gitDir);

async function runCommand(human: string, cmd) {
  if (verbose) {
    const out = await cmd.noThrow();
    if (out.code != 0) {
      exit(human);
    }
  } else {
    const out = await cmd.stdout("piped").stderr("piped").noThrow();
    if (out.code != 0) {
      $.logLight("stdout");
      $.logGroup();
      $.log(out.stdout);
      $.logGroupEnd();
      $.logLight("stderr");
      $.logGroup();
      $.log(out.stderr);
      $.logGroupEnd();
      exit(human);
    }
  }
}

async function buildGitCommit(progress, commit) {
  const tempDir = $.path(await Deno.makeTempDir());

  const gitInfo =
    await $`git log --pretty=oneline --abbrev-commit -n1 ${commit}`.stdout(
      "piped",
    ).stderr("piped").noThrow();
  if (gitInfo.code != 0) {
    $.log(gitInfo.stdout);
    $.log(gitInfo.stderr);
    exit(`Failed to get git info for commit ${commit}`);
  }

  const hash = gitInfo.stdout.split(" ")[0];
  progress.message(`${commit} is ${hash}`);

  progress.message(`clone ${hash}`);
  await runCommand(
    `Failed to clone commit ${commit}`,
    $`git clone ${gitDir} ${tempDir}`,
  );

  progress.message(`reset ${hash}`);
  await runCommand(
    `Failed to reset commit ${commit}`,
    $`git reset --hard ${hash}`.cwd(tempDir),
  );

  progress.message(`build ${hash} (please wait)`);
  const now = Date.now();
  const interval = setInterval(() => {
    const elapsed = Math.round((Date.now() - now) / 1000);
    progress.message(`build ${hash} (${elapsed}s)`);
  }, 100);
  try {
    if (profile === "debug") {
      await runCommand(
        `Failed to build commit ${commit}`,
        $`cargo build`.cwd(tempDir),
      );
    } else {
      await runCommand(
        `Failed to build commit ${commit}`,
        $`cargo build --profile ${profile}`.cwd(tempDir),
      );
    }
  } finally {
    clearInterval(interval);
  }
  const elapsed = Math.round((Date.now() - now) / 1000);

  let file;
  if (profile === "release") {
    file = `deno-${hash}`;
  } else {
    file = `deno-${profile}-${hash}`;
  }
  progress.message(`copy ${hash}`);
  await tempDir.join("target").join(profile).join("deno").copyFile(file);

  progress.message(`cleanup ${hash}`);
  await tempDir.remove({ recursive: true });

  progress.message("done");
  $.log(`Built ./${file} (${commit}) in ${elapsed}s: ${gitInfo.stdout}`);
}

const promises = [];
for (const arg of args) {
  if (verbose) {
    promises.push(buildGitCommit({ message() {} }, arg));
  } else {
    const progress = $.progress(`${arg}`);
    promises.push(progress.with(async () => {
      await buildGitCommit(progress, arg);
    }));
  }
}

await Promise.all(promises);
