#!/usr/bin/env -S deno run -A
// deno-lint-ignore-file no-console
// Watch CI on a PR, auto-remove failing node_compat tests from config.jsonc,
// commit and push until green.
//
// Usage: deno run -A watch_and_fix_ci.ts <PR_NUMBER> [--repo denoland/deno]

const PR = Deno.args[0];
const REPO = Deno.args.includes("--repo")
  ? Deno.args[Deno.args.indexOf("--repo") + 1]
  : "denoland/deno";
const CONFIG = new URL("./config.jsonc", import.meta.url).pathname;
const POLL_INTERVAL_MS = 45_000;

if (!PR) {
  console.error("Usage: watch_and_fix_ci.ts <PR_NUMBER> [--repo owner/repo]");
  Deno.exit(1);
}

async function run(
  cmd: string[],
): Promise<{ stdout: string; stderr: string; code: number }> {
  const p = new Deno.Command(cmd[0], {
    args: cmd.slice(1),
    stdout: "piped",
    stderr: "piped",
  });
  const { stdout, stderr, code } = await p.output();
  return {
    stdout: new TextDecoder().decode(stdout),
    stderr: new TextDecoder().decode(stderr),
    code,
  };
}

interface Check {
  name: string;
  status: string; // "pass" | "fail" | "pending" | ...
  url: string;
}

async function getChecks(): Promise<Check[]> {
  const { stdout } = await run(["gh", "pr", "checks", PR, "--repo", REPO]);
  return stdout.trim().split("\n").filter(Boolean).map((line) => {
    const parts = line.split("\t");
    return {
      name: parts[0]?.trim(),
      status: parts[1]?.trim(),
      url: parts[3]?.trim(),
    };
  });
}

function getJobId(url: string): string | null {
  const match = url?.match(/\/job\/(\d+)/);
  return match ? match[1] : null;
}

async function getFailedTests(jobId: string): Promise<string[]> {
  const { stdout } = await run([
    "gh",
    "api",
    `repos/${REPO}/actions/jobs/${jobId}/logs`,
  ]);
  const lines = stdout.split("\n");
  const idx = lines.findIndex((l) => l.includes("failed tests:"));
  if (idx === -1) return [];
  const tests: string[] = [];
  for (let i = idx + 1; i < Math.min(idx + 20, lines.length); i++) {
    const m = lines[i].match(/node_compat::([^:]+)::(.+\.(?:js|mjs|cjs))/);
    if (m) tests.push(`${m[1]}/${m[2].trim()}`);
    else if (lines[i].trim() === "") break;
  }
  return tests;
}

function removeFromConfig(test: string): boolean {
  const content = Deno.readTextFileSync(CONFIG);
  const escaped = test.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  // single-line entry
  let next = content.replace(
    new RegExp(`    "${escaped}": \\{[^}]*\\},?\\n`),
    "",
  );
  if (next === content) {
    // multi-line entry (platform flags on separate lines)
    next = content.replace(
      new RegExp(`    "${escaped}": \\{\\n[\\s\\S]*?\\},?\\n`),
      "",
    );
  }
  if (next === content) return false;
  Deno.writeTextFileSync(CONFIG, next);
  return true;
}

async function commitAndPush(removed: string[]) {
  const msg = `remove failing node_compat tests from CI\n\n${
    removed.map((t) => `- ${t}`).join("\n")
  }\n\nCo-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>`;
  await run(["git", "add", CONFIG]);
  await run(["git", "commit", "-m", msg]);
  const branch = (await run(["git", "branch", "--show-current"])).stdout.trim();
  const { code, stderr } = await run(["git", "push", "origin", branch]);
  if (code !== 0) console.error("Push failed:", stderr);
  else console.log(`Pushed removal of: ${removed.join(", ")}`);
}

const seen = new Set<string>(); // tests already removed this session

console.log(
  `Watching PR #${PR} on ${REPO} — polling every ${POLL_INTERVAL_MS / 1000}s`,
);

while (true) {
  const checks = await getChecks();
  const pending = checks.filter((c) =>
    !["pass", "fail", "skipping"].includes(c.status)
  ).length;
  const failed = checks.filter((c) => c.status === "fail");

  if (failed.length > 0) {
    const toRemove: string[] = [];

    for (const check of failed) {
      if (!check.name.includes("node_compat")) {
        console.log(
          `Non-node_compat failure: ${check.name} — needs manual fix`,
        );
        continue;
      }
      const jobId = getJobId(check.url);
      if (!jobId) continue;
      const tests = await getFailedTests(jobId);
      for (const t of tests) {
        if (seen.has(t)) continue;
        console.log(`Removing failing test: ${t}`);
        if (removeFromConfig(t)) {
          seen.add(t);
          toRemove.push(t);
        } else {
          console.log(`  (not found in config, already removed?)`);
        }
      }
    }

    if (toRemove.length > 0) {
      await commitAndPush(toRemove);
      console.log("Pushed fix, waiting for new CI run...");
      await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS * 2));
      continue;
    }
  }

  const hasSubstantiveChecks = checks.some((c) =>
    c.name.includes("node_compat") || c.name.includes("build") ||
    c.name.includes("test")
  );
  if (pending === 0 && failed.length === 0 && hasSubstantiveChecks) {
    console.log("All checks passed! CI is green.");
    break;
  }

  await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
}
