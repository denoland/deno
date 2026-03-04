#!/usr/bin/env -S deno run --allow-read --allow-env --allow-net
// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

// This script reads wpt_newly_passing.json and posts (or updates) a PR comment
// listing the WPT tests that are now passing but still expected to fail.

const COMMENT_MARKER = "<!-- wpt-newly-passing -->";

interface NewlyPassing {
  tests: string[];
  files: string[];
}

const jsonPath = Deno.args[0];
if (!jsonPath) {
  console.error("Usage: wpt_pr_comment.ts <wpt_newly_passing.json>");
  Deno.exit(1);
}

const token = Deno.env.get("GITHUB_TOKEN");
const repo = Deno.env.get("GITHUB_REPOSITORY");
const prNumber = Deno.env.get("PR_NUMBER");

if (!token || !repo || !prNumber) {
  console.error(
    "GITHUB_TOKEN, GITHUB_REPOSITORY, and PR_NUMBER env vars are required",
  );
  Deno.exit(1);
}

let data: NewlyPassing;
try {
  data = JSON.parse(await Deno.readTextFile(jsonPath));
} catch {
  console.log("Could not read newly passing tests file, skipping.");
  Deno.exit(0);
}

if (data.tests.length === 0 && data.files.length === 0) {
  console.log("No newly passing WPT tests found.");
  Deno.exit(0);
}

const lines: string[] = [
  COMMENT_MARKER,
  "### ⚠️ Newly Passing WPT Tests",
  "",
  "The following WPT tests are now passing but are still expected to fail.",
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

const commentBody = lines.join("\n");

const apiBase = `https://api.github.com/repos/${repo}`;
const headers = {
  "Authorization": `token ${token}`,
  "Content-Type": "application/json",
  "Accept": "application/vnd.github.v3+json",
};

// Check for an existing comment to update
const commentsResp = await fetch(
  `${apiBase}/issues/${prNumber}/comments?per_page=100`,
  { headers },
);
if (!commentsResp.ok) {
  console.error(
    `Failed to fetch comments: ${commentsResp.status} ${await commentsResp
      .text()}`,
  );
  Deno.exit(1);
}
const comments = await commentsResp.json();
const existing = comments.find((c: { body?: string }) =>
  c.body?.includes(COMMENT_MARKER)
);

if (existing) {
  console.log(`Updating existing comment ${existing.id}...`);
  const resp = await fetch(`${apiBase}/issues/comments/${existing.id}`, {
    method: "PATCH",
    headers,
    body: JSON.stringify({ body: commentBody }),
  });
  if (!resp.ok) {
    console.error(
      `Failed to update comment: ${resp.status} ${await resp.text()}`,
    );
    Deno.exit(1);
  }
  console.log("Comment updated.");
} else {
  console.log("Posting new comment...");
  const resp = await fetch(`${apiBase}/issues/${prNumber}/comments`, {
    method: "POST",
    headers,
    body: JSON.stringify({ body: commentBody }),
  });
  if (!resp.ok) {
    console.error(
      `Failed to post comment: ${resp.status} ${await resp.text()}`,
    );
    Deno.exit(1);
  }
  console.log("Comment posted.");
}
