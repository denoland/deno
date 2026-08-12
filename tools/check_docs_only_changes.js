// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

// Checks whether a pull request only touches files under the `doc/` directory.
// When that's the case, CI skips the build, test, bench and deno_core jobs and
// runs the `lint` job alone (markdown formatting + lint), since there is no
// code to compile or test.
//
// Usage: deno run --allow-run=git check_docs_only_changes.js <base_sha>
// Writes "docs_only=true" or "docs_only=false" to $GITHUB_OUTPUT when running
// in GitHub Actions.

const DOCS_DIR = "doc/";

const baseSha = Deno.args[0];
if (!baseSha) {
  console.error("Usage: check_docs_only_changes.js <base_sha>");
  Deno.exit(1);
}

// Get list of changed files between the base SHA and HEAD
const { code, stdout, stderr } = await Deno.spawnAndWait("git", [
  "diff",
  "--name-only",
  `${baseSha}..HEAD`,
]);

if (code !== 0) {
  console.error("git diff failed:", stderr);
  console.log("Defaulting to running the full CI (docs_only=false)");
  await writeOutput(false);
  Deno.exit(0);
}

const stdoutText = new TextDecoder().decode(stdout);
const changedFiles = stdoutText.trim().split("\n").filter(Boolean);

// Only treat the PR as docs-only when there is at least one changed file and
// every changed file lives under `doc/`. An empty diff defaults to running the
// full CI to be safe.
const docsOnly = changedFiles.length > 0 &&
  changedFiles.every((file) => file.startsWith(DOCS_DIR));

console.log(`Changed files (${changedFiles.length}):`);
for (const f of changedFiles) {
  console.log(`  ${f}`);
}
console.log(`docs_only: ${docsOnly}`);

await writeOutput(docsOnly);

async function writeOutput(docsOnly) {
  const outputFile = Deno.env.get("GITHUB_OUTPUT");
  if (outputFile) {
    await Deno.writeTextFile(outputFile, `docs_only=${docsOnly}\n`, {
      append: true,
    });
  }
}
