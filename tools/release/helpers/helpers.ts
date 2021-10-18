// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import type { DenoWorkspaceCrate } from "./deno_workspace.ts";

export function getCratesPublishOrder(crates: DenoWorkspaceCrate[]) {
  const pendingCrates = [...crates];
  const sortedCrates = [];

  while (pendingCrates.length > 0) {
    for (let i = pendingCrates.length - 1; i >= 0; i--) {
      const crate = pendingCrates[i];
      const hasPendingDependency = crate.getDependencies()
        .some((c) => pendingCrates.includes(c));
      if (!hasPendingDependency) {
        sortedCrates.push(crate);
        pendingCrates.splice(i, 1);
      }
    }
  }

  return sortedCrates;
}

export function getGitLogFromTag(directory: string, tagName: string) {
  return runCommand({
    cwd: directory,
    cmd: ["git", "log", "--oneline", `${tagName}..`],
  });
}

const IGNORED_COMMIT_PREFIX = [
  "build",
  "chore",
  "ci",
  "docs",
  "refactor",
  "test",
];

export function formatGitLogForMarkdown(text: string) {
  return text.split(/\r?\n/)
    .map((line) => line.replace(/^[a-f0-9]{9} /i, "").trim())
    .filter((l) => {
      return !IGNORED_COMMIT_PREFIX.some((prefix) => l.startsWith(prefix)) &&
        l.length > 0;
    })
    .sort()
    .map((line) => `- ${line}`)
    .join("\n");
}

export async function runCommand(params: {
  cwd: string;
  cmd: string[];
}) {
  const p = Deno.run({
    cwd: params.cwd,
    cmd: params.cmd,
    stderr: "piped",
    stdout: "piped",
  });

  const [status, stdout, stderr] = await Promise.all([
    p.status(),
    p.output(),
    p.stderrOutput(),
  ]);
  p.close();

  if (!status.success) {
    throw new Error(
      `Error executing ${params.cmd[0]}: ${new TextDecoder().decode(stderr)}`,
    );
  }

  return new TextDecoder().decode(stdout);
}

export async function withRetries<TReturn>(params: {
  action: () => Promise<TReturn>;
  retryCount: number;
  retryDelaySeconds: number;
}) {
  for (let i = 0; i < params.retryCount; i++) {
    if (i > 0) {
      console.log(
        `Failed. Trying again in ${params.retryDelaySeconds} seconds...`,
      );
      await delay(params.retryDelaySeconds * 1000);
      console.log(`Attempt ${i + 1}/${params.retryCount}...`);
    }
    try {
      return await params.action();
    } catch (err) {
      console.error(err);
    }
  }

  throw new Error(`Failed after ${params.retryCount} attempts.`);
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
