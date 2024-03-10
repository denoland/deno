// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
const prTitle = Deno.args[0];

if (prTitle == null) {
  Deno.exit(0); // not a PR
}

console.log("PR title:", prTitle);

// This is a release PR, so it's valid.
if (/^[^\s]+\.[^\s]+\.[^\s]+$/.test(prTitle)) {
  console.log("Valid.");
  Deno.exit(0);
}

const validPrefixes = [
  "chore",
  "fix",
  "feat",
  "perf",
  "ci",
  "cleanup",
  "docs",
  "bench",
  "build",
  "refactor",
  "test",
  // allow Revert PRs because it allows us to remove the landed
  // commit from the generated changelog
  "Revert ",
  // allow Reland PRs because when editing the changelog, it helps us identify
  // commits that were reverted, but then relanded
  "Reland ",
  // Allow landing breaking changes that are properly marked
  "BREAKING",
];

if (validPrefixes.some((prefix) => prTitle.startsWith(prefix))) {
  console.log("Valid.");
} else {
  console.error(
    "The PR title must start with one of the following prefixes:\n",
  );
  for (const prefix of validPrefixes) {
    console.error(`  - ${prefix}`);
  }
  console.error(
    "\nPlease fix the PR title according to https://www.conventionalcommits.org " +
      "then push an empty commit to reset the CI.",
  );
  Deno.exit(1);
}
