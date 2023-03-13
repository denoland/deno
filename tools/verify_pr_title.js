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
  "ci",
  "cleanup",
  "docs",
  "bench",
  "build",
  "refactor",
  "test",
];

if (validPrefixes.some((prefix) => prTitle.startsWith(prefix))) {
  console.log("Valid.");
} else {
  console.error(
    "The PR title must start with one of the following prefixes:\n",
  );
  for (prefix of validPrefixes) {
    console.error(`  - ${prefix}`);
  }
  Deno.exit(1);
}
