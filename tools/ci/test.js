/**
 * This script reads a cargo build manifest, and determines which built binaries
 * are test files that should be run. It then runs these test binaries and
 * proxies that test exit code.
 *
 * The manifest is read from `./target/{profile}/cargo_build_manifest.json`. It
 * is generated with: `cargo test --locked --no-run --message-format=json`.
 */

import { CargoBuildManifest } from "./_util.js";
import { dirname } from "../../test_util/std/path/mod.ts";

const profile = Deno.args[0];
const manifestPath = `./target/${profile}/cargo_build_manifest.json`;

const manifest = new CargoBuildManifest(manifestPath);
// We exclude proc macro tests, because these can not be run on a system that
// does not have `cargo` installed.
const tests = manifest.tests
  .filter((e) => !e.target.kind.includes("proc-macro"))
  .map((e) => [e.executable, e.manifest_path])
  .sort((a, b) => a[0].localeCompare(b[0]));

const shard = parseInt(Deno.args[1]);
const numShards = parseInt(Deno.args[2]);

const testsToRun = tests.filter((_, i) => i % numShards === shard - 1);

for (const [executable, manifestPath] of testsToRun) {
  console.log("%cRunning", "color: green; font-weight: bold;", executable);
  const rest = Deno.args.slice(3);
  const cwd = dirname(manifestPath);
  const proc = Deno.run({ cmd: [executable, ...rest], cwd });
  const { code } = await proc.status();
  if (code !== 0) {
    Deno.exit(code);
  }
  proc.close();
}
