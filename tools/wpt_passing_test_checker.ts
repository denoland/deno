#!/usr/bin/env -S deno run --allow-read --allow-env --allow-net
// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

// This script reads wpt_newly_passing.json (produced by the WPT test runner)
// and opens a GitHub issue listing WPT tests that are now passing but still
// marked as expected failures. It uses the Hashy service to avoid opening
// duplicate issues for the same set of newly passing tests.

const HASHY_URL = "https://hashy.deno.deno.net";
const HASHY_KEY = "wpt_newly_passing_hash";
const GITHUB_API = "https://api.github.com";
const REPO_OWNER = "denoland";
const REPO_NAME = "deno";

interface NewlyPassing {
  tests: string[];
  files: string[];
}

function getEnvOrExit(name: string): string {
  const value = Deno.env.get(name);
  if (!value) {
    console.error(`${name} environment variable is required`);
    Deno.exit(1);
  }
  return value;
}

const dryRun = Deno.args.includes("--dry-run");
const jsonPath = Deno.args.find((a) => !a.startsWith("--"));

if (!jsonPath) {
  console.error(
    "Usage: wpt_passing_test_checker.ts [--dry-run] <wpt_newly_passing.json>",
  );
  Deno.exit(1);
}

const token = dryRun ? Deno.env.get("GITHUB_TOKEN") : getEnvOrExit(
  "GITHUB_TOKEN",
);

let data: NewlyPassing;
try {
  data = JSON.parse(await Deno.readTextFile(jsonPath));
} catch (err) {
  if (err instanceof Deno.errors.NotFound) {
    console.log("Newly passing tests file not found, nothing to report.");
    Deno.exit(0);
  }
  console.error(`Failed to read ${jsonPath}:`, err);
  Deno.exit(1);
}

if (data.tests.length === 0 && data.files.length === 0) {
  console.log("No newly passing WPT tests found.");
  Deno.exit(0);
}

// Build a deterministic hash of the newly passing tests to deduplicate issues.
const contentToHash = JSON.stringify({
  tests: [...data.tests].sort(),
  files: [...data.files].sort(),
});
const hashBuffer = await crypto.subtle.digest(
  "SHA-256",
  new TextEncoder().encode(contentToHash),
);
const contentHash = Array.from(new Uint8Array(hashBuffer))
  .map((b) => b.toString(16).padStart(2, "0"))
  .join("");

// Check Hashy to see if we already reported this exact set.
try {
  const res = await fetch(`${HASHY_URL}/hashes/${HASHY_KEY}`, {
    signal: AbortSignal.timeout(5000),
  });
  if (res.ok) {
    const stored = await res.text();
    if (stored === contentHash) {
      console.log(
        "These newly passing tests were already reported. Skipping.",
      );
      Deno.exit(0);
    }
  }
} catch {
  console.warn("Could not check Hashy for deduplication, continuing anyway.");
}

// Build issue body
const lines: string[] = [
  "The following WPT tests are now passing on `main` but are still marked as expected failures.",
  "Please update the expectation files to reflect the new status.",
  "",
];

if (data.tests.length > 0) {
  lines.push("<details>");
  lines.push(
    `<summary>Newly passing test cases (${data.tests.length})</summary>`,
  );
  lines.push("");
  for (const t of data.tests) {
    lines.push(`- \`${t}\``);
  }
  lines.push("");
  lines.push("</details>");
  lines.push("");
}

if (data.files.length > 0) {
  lines.push("<details>");
  lines.push(
    `<summary>Newly passing test files (${data.files.length})</summary>`,
  );
  lines.push("");
  for (const f of data.files) {
    lines.push(`- \`${f}\``);
  }
  lines.push("");
  lines.push("</details>");
  lines.push("");
}

lines.push("To update expectations, run:");
lines.push("");
lines.push("```bash");
lines.push("./tests/wpt/wpt.ts update --all");
lines.push("```");

const totalCount = data.tests.length + data.files.length;
const issueTitle = `wpt: ${totalCount} newly passing test${
  totalCount !== 1 ? "s" : ""
} need expectation updates`;
const issueBody = lines.join("\n");

if (dryRun) {
  console.log("=== DRY RUN ===");
  console.log(`Title: ${issueTitle}`);
  console.log(`Body:\n${issueBody}`);
  console.log(`Content hash: ${contentHash}`);
  Deno.exit(0);
}

// Open the issue
const headers = {
  "Authorization": `Bearer ${token}`,
  "Content-Type": "application/json",
  "Accept": "application/vnd.github.v3+json",
};

console.log("Opening GitHub issue...");
const resp = await fetch(
  `${GITHUB_API}/repos/${REPO_OWNER}/${REPO_NAME}/issues`,
  {
    method: "POST",
    headers,
    body: JSON.stringify({
      title: issueTitle,
      body: issueBody,
      labels: ["wpt"],
    }),
  },
);

if (!resp.ok) {
  console.error(
    `Failed to create issue: ${resp.status} ${await resp.text()}`,
  );
  Deno.exit(1);
}

const issue = await resp.json();
console.log(`Issue created: ${issue.html_url}`);

// Save hash to Hashy so we don't open a duplicate next time.
try {
  await fetch(`${HASHY_URL}/hashes/${HASHY_KEY}`, {
    method: "PUT",
    body: contentHash,
    signal: AbortSignal.timeout(5000),
  });
  console.log("Saved content hash to Hashy.");
} catch {
  console.warn("Failed to save content hash to Hashy.");
}
