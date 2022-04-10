// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { semver } from "./deps.ts";

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

export async function runCommandWithOutput(params: {
  cwd: string;
  cmd: string[];
}) {
  const p = Deno.run({
    cwd: params.cwd,
    cmd: params.cmd,
    stderr: "inherit",
    stdout: "inherit",
  });

  const status = await p.status();
  p.close();

  if (!status.success) {
    throw new Error(
      `Error executing ${params.cmd[0]}. Code: ${status.code}`,
    );
  }
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

export function existsSync(path: string): boolean {
  try {
    Deno.lstatSync(path);
    return true;
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      return false;
    }
    throw err;
  }
}

export interface GitLogLine {
  rev: string;
  message: string;
}

export class GitLogOutput {
  lines: GitLogLine[];

  constructor(lines: GitLogLine[]) {
    this.lines = lines;
  }

  formatForReleaseMarkdown() {
    const IGNORED_COMMIT_PREFIX = [
      "bench",
      "build",
      "chore",
      "ci",
      "cleanup",
      "docs",
      "refactor",
      "test",
    ];
    return this.lines
      .filter((l) => {
        // don't include version commits
        if (/^v?[0-9]+\.[0-9]+\.[0-9]+/.test(l.message)) {
          return false;
        }

        return !IGNORED_COMMIT_PREFIX
          .some((prefix) => l.message.startsWith(prefix)) &&
          l.message.length > 0;
      })
      .map((line) => `- ${line.message}`)
      .sort()
      .join("\n");
  }
}

export class GitTags {
  #tags: string[];

  constructor(tags: string[]) {
    this.#tags = tags;
  }

  /** Gets the tags that are for a version. */
  getGitVersionTags() {
    const result = [];
    for (const name of this.#tags) {
      const version = semver.parse(name.replace(/^v/, ""));
      if (version != null) {
        result.push({
          name,
          version,
        });
      }
    }
    return result;
  }

  /** Gets if the most recent version tag uses a `v` prefix. */
  usesVersionVPrefix() {
    const versionTags = this.getGitVersionTags();
    versionTags.sort((a, b) => a.version.compare(b.version));
    const mostRecentTag = versionTags[versionTags.length - 1];
    return mostRecentTag?.name.startsWith("v") ?? false;
  }

  has(tagName: string) {
    return this.#tags.includes(tagName);
  }

  getTagNameForVersion(version: string) {
    version = version.replace(/^v/, "");
    return this.usesVersionVPrefix() ? `v${version}` : version;
  }

  getPreviousVersionTag(version: string) {
    const v = semver.parse(version);
    if (v == null) {
      throw new Error(`Provided version was not a version: ${version}`);
    }
    let previousVersion;
    for (const tagInfo of this.getGitVersionTags()) {
      const isGtPrevious = previousVersion == null ||
        previousVersion.version.compare(tagInfo.version) < 0;
      if (isGtPrevious && tagInfo.version.compare(v) < 0) {
        previousVersion = tagInfo;
      }
    }
    return previousVersion?.name;
  }
}

export function containsVersion(text: string) {
  return /[0-9]+\.[0-9]+\.[0-9]+/.test(text);
}
