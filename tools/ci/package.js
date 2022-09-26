/**
 * This script reads a cargo build manifest, and determines which packages need
 * to be packaged.
 *
 * The manifest is read from `./target/{profile}/cargo_build_manifest.json`. It
 * is generated with: `cargo test --locked --no-run --message-format=json`.
 */

import { CargoBuildManifest } from "./_util.js";

const profile = Deno.args[0];
const manifestPath = `./target/${profile}/cargo_build_manifest.json`;

const manifest = new CargoBuildManifest(manifestPath);
const executables = [
  ...new Set([...manifest.bins, ...manifest.tests, ...manifest.benches]),
];
const artifacts = executables.map((e) => e.executable.replace(Deno.cwd(), "."));
artifacts.push(manifestPath);
const cdylibs = manifest.cdylibs
  .filter((e) => e.manifest_path.startsWith(Deno.cwd()))
  .flatMap((e) => e.filenames.map((path) => path.replace(Deno.cwd(), ".")));
artifacts.push(...cdylibs);

if (Deno.build.os === "darwin") {
  for (const artifact of artifacts) {
    console.log(`$ file ${artifact}`);
    await Deno.spawn("file", {
      args: [artifact],
      stdout: "inherit",
      stderr: "inherit",
    });
  }
}

const tar = Deno.build.os === "darwin" ? "gtar" : "tar";
const proc = Deno.run({
  cmd: [tar, "-Izstd", "-cvf", "artifacts.tar.zst", ...artifacts],
});
const { success } = await proc.status();
if (!success) {
  throw `Failed to package artifacts.tar.zst`;
}
