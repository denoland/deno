#!/usr/bin/env -S deno run --unstable --allow-read --allow-run
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { join, ROOT_PATH as ROOT } from "./util.js";

async function bashOut(subcmd) {
  const p = Deno.run({
    cmd: ["bash", "-c", subcmd],
    stdout: "piped",
    stderr: "null",
  });

  // Check for failure
  const { success } = await p.status();
  if (!success) {
    throw new Error("subcmd failed");
  }
  // Gather output
  const output = new TextDecoder().decode(await p.output());
  // Cleanup
  p.close();

  return output.trim();
}

async function bashThrough(subcmd, opts = {}) {
  const p = Deno.run({ ...opts, cmd: ["bash", "-c", subcmd] });

  // Exit process on failure
  const { success, code } = await p.status();
  if (!success) {
    Deno.exit(code);
  }
  // Cleanup
  p.close();
}

async function availableBenches() {
  // TODO(AaronO): maybe reimplement with fs.walk
  // it's important to prune the walked tree so this is fast (<50ms)
  const prunedDirs = ["third_party", ".git", "target", "docs", "test_util"];
  const pruneExpr = prunedDirs.map((d) => `-path ${ROOT}/${d}`).join(" -o ");
  return (await bashOut(`
  find ${ROOT} -type d \
      \\( ${pruneExpr} \\) \
      -prune -false -o  \
      -path "${ROOT}/*/benches/*" -type f -name "*.rs" \
      | xargs basename | cut -f1 -d.
  `)).split("\n");
}

function latestBenchBin(name) {
  return bashOut(`ls -t "${ROOT}/target/release/deps/${name}"* | head -n 1`);
}

function runFlamegraph(benchBin, benchFilter, outputFile) {
  return bashThrough(
    `sudo -E flamegraph -o ${outputFile} ${benchBin} ${benchFilter}`,
    // Set $PROFILING env so benches can improve their flamegraphs
    { env: { "PROFILING": "1" } },
  );
}

async function binExists(bin) {
  try {
    await bashOut(`which ${bin}`);
    return true;
  } catch (_) {
    return false;
  }
}

async function main() {
  const { 0: benchName, 1: benchFilter } = Deno.args;
  // Print usage if no bench specified
  if (!benchName) {
    console.log("flamebench <bench_name> [bench_filter]");
    // Also show available benches
    console.log("\nAvailable benches:");
    const benches = await availableBenches();
    console.log(benches.join("\n"));
    return Deno.exit(1);
  }

  // List available benches, hoping we don't have any benches called "ls" :D
  if (benchName === "ls") {
    const benches = await availableBenches();
    console.log(benches.join("\n"));
    return;
  }

  // Ensure flamegraph is installed
  if (!await binExists("flamegraph")) {
    console.log(
      "flamegraph (https://github.com/flamegraph-rs/flamegraph) not found, please run:",
    );
    console.log();
    console.log("cargo install flamegraph");
    return Deno.exit(1);
  }

  // Build bench with frame pointers
  await bashThrough(
    `RUSTFLAGS='-C force-frame-pointers=y' cargo build --release --bench ${benchName}`,
  );

  // Get the freshly built bench binary
  const benchBin = await latestBenchBin(benchName);

  // Run flamegraph
  const outputFile = join(ROOT, "flamebench.svg");
  await runFlamegraph(benchBin, benchFilter ?? "", outputFile);

  // Open flamegraph (in your browser / SVG viewer)
  if (await binExists("open")) {
    await bashThrough(`open ${outputFile}`);
  }
}
// Run
await main();
