/**
 * This script reads a cargo build manifest, and determines which packages need
 * to be packaged.
 *
 * The manifest is read from `./target/{profile}/cargo_build_manifest.json`. It
 * is generated with: `cargo test --locked --no-run --message-format=json`.
 *
 * It creates 3 archives packages:
 *  - `./artifacts_[1-3].tar` - 3 numbered tar archives containing all other created artifacts
 */

import { CargoBuildManifest } from "./_util.js";

const ARCHIVE_COUNT = 3;

const profile = Deno.args[0];
const manifestPath = `./target/${profile}/cargo_build_manifest.json`;

function relative(p) {
  return p.replace(Deno.cwd(), ".");
}

const manifest = new CargoBuildManifest(manifestPath);
const executables = new Set([
  ...manifest.bins,
  ...manifest.tests,
  ...manifest.benches,
]);

const artifacts = [...executables].map((e) => relative(e.executable));
artifacts.push(manifestPath);
const cdylibs = manifest.cdylibs
  .filter((e) => e.manifest_path.startsWith(Deno.cwd()))
  .flatMap((e) => e.filenames.map((f) => relative(f)));
artifacts.push(...cdylibs);

const artifactsPerArchive = Math.ceil(artifacts.length / ARCHIVE_COUNT);

for (let i = 0; i < 3; i += 1) {
  const path = `artifacts_${i + 1}.tar`;
  const files = artifacts.slice(
    i * artifactsPerArchive,
    (i + 1) * artifactsPerArchive,
  );
  const tar = Deno.build.os === "darwin" ? "gtar" : "tar";
  const proc = Deno.run({
    cmd: [tar, "-cvf", path, ...files],
  });
  const { success } = await proc.status();
  if (!success) {
    throw `Failed to package ${files}`;
  }
}
