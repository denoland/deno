// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

// Checks if any files changed relative to the base branch belong to
// deno_core-related package directories (or root Cargo files). Used in CI
// to decide whether to run deno_core tests.
//
// Usage: deno run --allow-run=git check_deno_core_changes.js <base_sha>
// Writes "skip_deno_core_test=true" or "skip_deno_core_test=false" to
// $GITHUB_OUTPUT when running in GitHub Actions.

const DENO_CORE_PACKAGE_DIRS = [
  "libs/core_testing",
  "libs/core",
  "libs/core/examples/snapshot",
  "libs/dcore",
  "libs/ops",
  "libs/ops/compile_test_runner",
  "libs/serde_v8",
];

const baseSha = Deno.args[0];
if (!baseSha) {
  console.error("Usage: check_deno_core_changes.js <base_sha>");
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
  console.log("Defaulting to running deno_core tests (skip=false)");
  await writeOutput(false);
  Deno.exit(0);
}

const stdoutText = new TextDecoder().decode(stdout);
const changedFiles = stdoutText.trim().split("\n").filter(Boolean);

const denoCoreChanged = changedFiles.some((file) =>
  DENO_CORE_PACKAGE_DIRS.some((dir) => file.startsWith(dir + "/")) ||
  file === "Cargo.lock" ||
  file === "Cargo.toml"
);

const skip = !denoCoreChanged;
console.log(`Changed files (${changedFiles.length}):`);
for (const f of changedFiles) {
  console.log(`  ${f}`);
}
console.log(`Deno core changed: ${denoCoreChanged}`);
console.log(`skip_deno_core_test: ${skip}`);

await writeOutput(skip);

async function writeOutput(skip) {
  const outputFile = Deno.env.get("GITHUB_OUTPUT");
  if (outputFile) {
    await Deno.writeTextFile(outputFile, `skip_deno_core_test=${skip}\n`, {
      append: true,
    });
  }
}
