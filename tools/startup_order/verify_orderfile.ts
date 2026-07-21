#!/usr/bin/env -S deno run --allow-read --allow-run
// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-console camelcase

import { join, resolve } from "node:path";

const decoder = new TextDecoder();
// These compare the two binaries built from the same revision. Symbol counts
// and performance measurements remain report telemetry.
const MINIMUM_MATCHED_RATIO = 0.9;
// If pass one already follows nearly all of the requested sequence, pass two
// need not improve it; it only must not materially regress conformance.
const ALREADY_ORDERED_RATIO = 0.9;
const ALREADY_ORDERED_TOLERANCE = 0.02;
const SUPPORTED_TARGETS = new Set([
  "aarch64-apple-darwin",
  "aarch64-unknown-linux-gnu",
  "x86_64-unknown-linux-gnu",
]);

interface Options {
  baselineBinary: string;
  binary: string;
  order?: string;
  output?: string;
  target: string;
}

function hostTarget(): string {
  if (Deno.build.os === "darwin") {
    return `${Deno.build.arch}-apple-darwin`;
  }
  if (Deno.build.os === "linux") {
    return `${Deno.build.arch}-unknown-linux-gnu`;
  }
  throw new Error(`unsupported host ${Deno.build.os}-${Deno.build.arch}`);
}

function usage(): never {
  console.error(
    "usage: verify_orderfile.ts --baseline-binary PATH --binary PATH " +
      "[--order PATH] [--target TRIPLE] [--output PATH]",
  );
  Deno.exit(2);
}

function parseArgs(args: string[]): Options {
  let baselineBinary: string | undefined;
  let binary: string | undefined;
  let order: string | undefined;
  let output: string | undefined;
  let target = hostTarget();
  for (let index = 0; index < args.length; index++) {
    const flag = args[index];
    if (flag === "--help" || flag === "-h") usage();
    const value = args[++index];
    if (value === undefined) usage();
    if (flag === "--baseline-binary") {
      baselineBinary = resolve(value);
    } else if (flag === "--binary") {
      binary = resolve(value);
    } else if (flag === "--order") {
      order = resolve(value);
    } else if (flag === "--output") {
      output = resolve(value);
    } else if (flag === "--target") {
      target = value;
    } else {
      usage();
    }
  }
  if (baselineBinary === undefined || binary === undefined) usage();
  return { baselineBinary, binary, order, output, target };
}

async function sha256(bytes: Uint8Array<ArrayBuffer>): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

async function newestCargoOrder(target: string): Promise<string> {
  const buildRoot = resolve("target", "release", "build");
  const name = `startup-order-${target}.order`;
  const candidates: Array<{ path: string; modified: number }> = [];
  for await (const entry of Deno.readDir(buildRoot)) {
    if (!entry.isDirectory || !entry.name.startsWith("deno-")) continue;
    const path = join(buildRoot, entry.name, "out", name);
    try {
      const stat = await Deno.stat(path);
      candidates.push({ path, modified: stat.mtime?.getTime() ?? 0 });
    } catch (error) {
      if (!(error instanceof Deno.errors.NotFound)) throw error;
    }
  }
  candidates.sort((left, right) => right.modified - left.modified);
  if (candidates.length === 0) {
    throw new Error(
      `could not find ${name} under ${buildRoot}; pass --order explicitly`,
    );
  }
  return candidates[0].path;
}

async function loadTextSymbols(
  binary: string,
  target: string,
): Promise<Map<string, number>> {
  const command = target.endsWith("apple-darwin") ? "xcrun" : "nm";
  const args = target.endsWith("apple-darwin")
    ? ["llvm-nm", "--defined-only", "--numeric-sort", binary]
    : ["--defined-only", "--numeric-sort", binary];
  const output = await new Deno.Command(command, {
    args,
    stdout: "piped",
    stderr: "piped",
  }).output();
  if (!output.success) {
    throw new Error(
      `${command} exited ${output.code}: ${decoder.decode(output.stderr)}`,
    );
  }
  const symbols = new Map<string, number>();
  for (const line of decoder.decode(output.stdout).split("\n")) {
    const match = /^([0-9a-fA-F]+)\s+[tT]\s+(.+)$/.exec(line);
    if (match !== null && !symbols.has(match[2])) {
      symbols.set(match[2], Number.parseInt(match[1], 16));
    }
  }
  if (symbols.size === 0) {
    throw new Error(`found no text symbols in ${binary}; verify before strip`);
  }
  return symbols;
}

function longestNondecreasingLength(values: number[]): number {
  const tails: number[] = [];
  for (const value of values) {
    let low = 0;
    let high = tails.length;
    while (low < high) {
      const middle = (low + high) >>> 1;
      if (tails[middle] <= value) {
        low = middle + 1;
      } else {
        high = middle;
      }
    }
    tails[low] = value;
  }
  return tails.length;
}

function measureOrder(
  orderNames: string[],
  symbols: Map<string, number>,
) {
  const addresses = orderNames.flatMap((name) => {
    const address = symbols.get(name);
    return address === undefined ? [] : [address];
  });
  const orderedLength = longestNondecreasingLength(addresses);
  return {
    matchedSymbols: addresses.length,
    missingSymbols: orderNames.length - addresses.length,
    matchedRatio: addresses.length / orderNames.length,
    orderedSymbols: orderedLength,
    orderedRatio: addresses.length === 0 ? 0 : orderedLength / addresses.length,
  };
}

if (import.meta.main) {
  const options = parseArgs(Deno.args);
  if (!SUPPORTED_TARGETS.has(options.target)) {
    throw new Error(`startup ordering is unsupported for ${options.target}`);
  }

  const orderPath = options.order ?? await newestCargoOrder(options.target);
  const orderBytes = await Deno.readFile(orderPath);
  const orderHash = await sha256(orderBytes);
  const orderNames = decoder.decode(orderBytes).split("\n")
    .filter((line) => line.length > 0 && !line.startsWith("#"));
  if (orderNames.length === 0) {
    throw new Error("order file contained no symbols");
  }
  const duplicateCount = orderNames.length - new Set(orderNames).size;
  const [baselineSymbols, symbols] = await Promise.all([
    loadTextSymbols(options.baselineBinary, options.target),
    loadTextSymbols(options.binary, options.target),
  ]);
  const baseline = measureOrder(orderNames, baselineSymbols);
  const linked = measureOrder(orderNames, symbols);
  const baselineAlreadyOrdered = baseline.orderedRatio >= ALREADY_ORDERED_RATIO;
  const orderingImproved = linked.orderedRatio > baseline.orderedRatio;
  const report = {
    target: options.target,
    baseline_binary: options.baselineBinary,
    binary: options.binary,
    order_file: orderPath,
    order_sha256: orderHash,
    requested_symbols: orderNames.length,
    duplicate_symbols: duplicateCount,
    minimum_matched_ratio: MINIMUM_MATCHED_RATIO,
    baseline: {
      matched_symbols: baseline.matchedSymbols,
      missing_symbols: baseline.missingSymbols,
      matched_ratio: baseline.matchedRatio,
      longest_nondecreasing_symbols: baseline.orderedSymbols,
      ordered_ratio: baseline.orderedRatio,
      already_substantially_ordered: baselineAlreadyOrdered,
    },
    linked: {
      matched_symbols: linked.matchedSymbols,
      missing_symbols: linked.missingSymbols,
      matched_ratio: linked.matchedRatio,
      longest_nondecreasing_symbols: linked.orderedSymbols,
      ordered_ratio: linked.orderedRatio,
      improved_over_baseline: orderingImproved,
    },
  };

  console.log(JSON.stringify(report, null, 2));
  if (options.output !== undefined) {
    await Deno.writeTextFile(
      options.output,
      `${JSON.stringify(report, null, 2)}\n`,
    );
  }

  const failures: string[] = [];
  if (duplicateCount !== 0) {
    failures.push(`order file contains ${duplicateCount} duplicate symbols`);
  }
  if (baseline.matchedRatio < MINIMUM_MATCHED_RATIO) {
    failures.push(
      `baseline matched ratio ${baseline.matchedRatio.toFixed(4)} is below ` +
        MINIMUM_MATCHED_RATIO.toFixed(4),
    );
  }
  if (linked.matchedRatio < MINIMUM_MATCHED_RATIO) {
    failures.push(
      `linked matched ratio ${linked.matchedRatio.toFixed(4)} is below ` +
        MINIMUM_MATCHED_RATIO.toFixed(4),
    );
  }
  if (
    baselineAlreadyOrdered &&
    linked.orderedRatio <
      baseline.orderedRatio - ALREADY_ORDERED_TOLERANCE
  ) {
    failures.push(
      `baseline was already substantially ordered at ` +
        `${baseline.orderedRatio.toFixed(4)}, but relinking reduced it to ` +
        linked.orderedRatio.toFixed(4),
    );
  } else if (!baselineAlreadyOrdered && !orderingImproved) {
    failures.push(
      `relinking did not improve ordered ratio beyond baseline ` +
        baseline.orderedRatio.toFixed(4),
    );
  }
  if (failures.length > 0) {
    throw new Error(failures.join("; "));
  }
}
