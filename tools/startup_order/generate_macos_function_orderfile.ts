#!/usr/bin/env -S deno run --v8-flags=--jitless --allow-env --allow-read --allow-run --allow-write
// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-console camelcase
/**
 * Generate a macOS arm64 linker order from exact first function entries.
 *
 * It injects orderfile_function_tracer_macos.c to breakpoint each
 * LC_FUNCTION_STARTS entry, restore the entry on first execution, and record
 * the exact address.
 */

import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const decoder = new TextDecoder();
const TRACE_MAGIC = 0x44454e4f46554e43n;
const TRACE_VERSION = 1n;
const HEADER_WORDS = 5;
const COMMAND_TIMEOUT_MS = 120_000;

type WorkloadProfile = "run-first" | "timer-free-first";

interface Options {
  binary: string;
  output: string;
  tracerSource: string;
  repeats: number;
  workloadProfile: WorkloadProfile;
}

interface Workload {
  name: string;
  args: string[];
  cwd: string;
}

function usage(): never {
  console.error(
    "usage: generate_macos_function_orderfile.ts --binary PATH --output PATH " +
      "[--tracer-source PATH] [--repeats N] " +
      "[--workload-profile run-first|timer-free-first]",
  );
  Deno.exit(2);
}

function parseArgs(args: string[]): Options {
  let binary: string | undefined;
  let output: string | undefined;
  let tracerSource = join(
    dirname(fileURLToPath(import.meta.url)),
    "orderfile_function_tracer_macos.c",
  );
  let repeats = 3;
  let workloadProfile: WorkloadProfile = "run-first";
  for (let index = 0; index < args.length; index += 2) {
    const flag = args[index];
    const value = args[index + 1];
    if (value === undefined) usage();
    if (flag === "--binary") {
      binary = value;
    } else if (flag === "--output") {
      output = value;
    } else if (flag === "--tracer-source") {
      tracerSource = value;
    } else if (flag === "--repeats") {
      repeats = Number(value);
      if (!Number.isSafeInteger(repeats) || repeats <= 0) usage();
    } else if (flag === "--workload-profile") {
      if (value !== "run-first" && value !== "timer-free-first") usage();
      workloadProfile = value;
    } else {
      usage();
    }
  }
  if (binary === undefined || output === undefined) usage();
  return {
    binary: resolve(binary),
    output: resolve(output),
    tracerSource: resolve(tracerSource),
    repeats,
    workloadProfile,
  };
}

async function runCommand(
  argv: string[],
  options: {
    cwd?: string;
    env?: Record<string, string>;
    timeoutMs?: number;
  } = {},
): Promise<Deno.CommandOutput> {
  const [command, ...args] = argv;
  const child = new Deno.Command(command, {
    args,
    cwd: options.cwd,
    env: options.env,
    stdin: "null",
    stdout: "piped",
    stderr: "piped",
  }).spawn();
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    try {
      child.kill("SIGKILL");
    } catch {
      // The process exited between the timeout and kill.
    }
  }, options.timeoutMs ?? COMMAND_TIMEOUT_MS);
  const result = await child.output();
  clearTimeout(timer);
  if (timedOut) {
    throw new Error(`command timed out: ${argv.join(" ")}`);
  }
  if (!result.success) {
    throw new Error(
      `command exited ${result.code}: ${argv.join(" ")}\n` +
        `stdout:\n${decoder.decode(result.stdout)}\n` +
        `stderr:\n${decoder.decode(result.stderr)}`,
    );
  }
  return result;
}

async function writeFixtures(
  root: string,
  profile: WorkloadProfile,
): Promise<Workload[]> {
  const empty = join(root, "empty.ts");
  const timer = join(root, "timer.js");
  const timerFree = join(root, "timer_free.js");
  const hello = join(root, "hello.ts");
  const server = join(root, "server.ts");
  const test = join(root, "example_test.ts");
  const node = join(root, "node.ts");
  const format = join(root, "format_me.ts");
  await Promise.all([
    Deno.writeTextFile(empty, "void 0;\n"),
    Deno.writeTextFile(timer, "setTimeout(() => {}, 0);\n"),
    Deno.writeTextFile(
      timerFree,
      "Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 1);\n",
    ),
    Deno.writeTextFile(
      hello,
      "const greet = (name: string): string => `hello ${name}`;\n" +
        'console.log(greet("world"));\n',
    ),
    Deno.writeTextFile(
      server,
      "const abort = new AbortController();\n" +
        "const server = Deno.serve(\n" +
        "  { port: 0, signal: abort.signal },\n" +
        '  () => new Response("ok"),\n' +
        ");\n" +
        "await (await fetch(`http://127.0.0.1:${server.addr.port}/`)).text();\n" +
        "abort.abort();\n" +
        "await server.finished;\n",
    ),
    Deno.writeTextFile(
      test,
      'Deno.test("passes", () => {\n' +
        '  if (1 + 1 !== 2) throw new Error("bad arithmetic");\n' +
        "});\n",
    ),
    Deno.writeTextFile(
      node,
      'import { createHash } from "node:crypto";\n' +
        'console.log(createHash("sha256").update("x").digest("hex"));\n',
    ),
    Deno.writeTextFile(
      format,
      "export const value = { nested: [1, 2, 3] };\n",
    ),
  ]);

  const runEmpty = {
    name: "deno run empty",
    args: ["run", "--quiet", empty],
    cwd: root,
  };
  const runTimerFree = {
    name: "deno run timer-free",
    args: ["run", "--quiet", timerFree],
    cwd: root,
  };
  const remaining: Workload[] = [
    { name: "deno run hello", args: ["run", "--quiet", hello], cwd: root },
    {
      name: "deno run timer",
      args: ["run", "--quiet", timer],
      cwd: root,
    },
    {
      name: "deno serve",
      args: ["run", "--quiet", "--allow-net", server],
      cwd: root,
    },
    { name: "deno test", args: ["test", "--quiet", test], cwd: root },
    {
      name: "deno node:crypto",
      args: ["run", "--quiet", node],
      cwd: root,
    },
    { name: "deno fmt", args: ["fmt", "--check", format], cwd: root },
  ];
  return profile === "run-first"
    ? [runEmpty, ...remaining]
    : [runTimerFree, runEmpty, ...remaining];
}

async function loadSymbols(
  binary: string,
): Promise<Map<number, string[]>> {
  const result = await runCommand([
    "xcrun",
    "llvm-nm",
    "--defined-only",
    "--numeric-sort",
    binary,
  ]);
  const symbols = new Map<number, string[]>();
  for (const line of decoder.decode(result.stdout).split("\n")) {
    const match = /^([0-9a-fA-F]+)\s+[tT]\s+(.+)$/.exec(line);
    if (match === null || match[2] === "__mh_execute_header") continue;
    const address = Number.parseInt(match[1], 16);
    const names = symbols.get(address);
    if (names === undefined) {
      symbols.set(address, [match[2]]);
    } else {
      names.push(match[2]);
    }
  }
  if (symbols.size === 0) {
    throw new Error(`llvm-nm found no text symbols in ${binary}`);
  }
  return symbols;
}

function parseTrace(raw: Uint8Array): {
  slide: number;
  totalStarts: number;
  addresses: number[];
} {
  if (raw.byteLength < HEADER_WORDS * 8) {
    throw new Error("truncated function trace");
  }
  const view = new DataView(raw.buffer, raw.byteOffset, raw.byteLength);
  if (
    view.getBigUint64(0, true) !== TRACE_MAGIC ||
    view.getBigUint64(8, true) !== TRACE_VERSION
  ) {
    throw new Error("invalid function trace header");
  }
  const slide = Number(view.getBigUint64(16, true));
  const totalStarts = Number(view.getBigUint64(24, true));
  const count = Number(view.getBigUint64(32, true));
  if (
    !Number.isSafeInteger(count) ||
    count < 0 ||
    count > totalStarts ||
    raw.byteLength < (HEADER_WORDS + count) * 8
  ) {
    throw new Error("invalid function trace count");
  }
  const addresses: number[] = [];
  for (let index = 0; index < count; index++) {
    addresses.push(
      Number(view.getBigUint64((HEADER_WORDS + index) * 8, true)),
    );
  }
  return { slide, totalStarts, addresses };
}

function intersectionSize(sets: Set<number>[]): number {
  return [...sets[0]].filter((address) => sets.every((set) => set.has(address)))
    .length;
}

async function generate(options: Options) {
  if (Deno.build.os !== "darwin" || Deno.build.arch !== "aarch64") {
    throw new Error("exact function tracing requires arm64 macOS");
  }
  const binary = await Deno.realPath(options.binary);
  const tracerSource = await Deno.realPath(options.tracerSource);
  const runnerSource = await Deno.realPath(
    join(dirname(tracerSource), "orderfile_trace_runner.c"),
  );
  await Deno.mkdir(dirname(options.output), { recursive: true });

  const root = await Deno.makeTempDir({ prefix: "deno-function-order-" });
  const tracer = join(root, "function_tracer.dylib");
  const runner = join(root, "trace_runner");
  const denoDir = join(root, "deno-dir");
  await Deno.mkdir(denoDir);
  try {
    await runCommand([
      "xcrun",
      "clang",
      "-O2",
      "-dynamiclib",
      "-fPIC",
      "-o",
      tracer,
      tracerSource,
    ]);
    await runCommand(["xcrun", "clang", "-O2", "-o", runner, runnerSource]);

    const symbols = await loadSymbols(binary);
    const workloads = await writeFixtures(root, options.workloadProfile);
    const orderedNames: string[] = [];
    const seenNames = new Set<string>();
    const seenAddresses = new Set<number>();
    const workloadReports = [];
    let totalStarts: number | undefined;

    for (
      let workloadIndex = 0;
      workloadIndex < workloads.length;
      workloadIndex++
    ) {
      const workload = workloads[workloadIndex];
      const traces: number[][] = [];
      for (let repeat = 0; repeat < options.repeats; repeat++) {
        const tracePath = join(
          root,
          `trace-${workloadIndex}-${repeat}.bin`,
        );
        await runCommand([
          runner,
          String(Deno.pid),
          binary,
          ...workload.args,
        ], {
          cwd: workload.cwd,
          env: {
            ...Deno.env.toObject(),
            DENO_ORDER_RUNNER_PRELOAD: tracer,
            DENO_FUNCTION_TRACE_OUT: tracePath,
            DENO_DIR: denoDir,
            DENO_NO_UPDATE_CHECK: "1",
            DENO_NO_PACKAGE_JSON: "1",
            NO_COLOR: "1",
          },
        });
        const trace = parseTrace(await Deno.readFile(tracePath));
        totalStarts ??= trace.totalStarts;
        if (trace.totalStarts !== totalStarts) {
          throw new Error("LC_FUNCTION_STARTS count changed between traces");
        }
        traces.push(trace.addresses);
      }

      const repeatSets = traces.map((trace) => new Set(trace));
      const workloadSequence: number[] = [];
      const workloadSeen = new Set<number>();
      for (const trace of traces) {
        for (const address of trace) {
          if (!workloadSeen.has(address)) {
            workloadSeen.add(address);
            workloadSequence.push(address);
          }
        }
      }

      let newAddresses = 0;
      let newNames = 0;
      let addressesWithoutSymbols = 0;
      for (const address of workloadSequence) {
        if (!seenAddresses.has(address)) {
          seenAddresses.add(address);
          newAddresses++;
        }
        const names = symbols.get(address);
        if (names === undefined) {
          addressesWithoutSymbols++;
          continue;
        }
        for (const name of names) {
          if (!seenNames.has(name)) {
            seenNames.add(name);
            orderedNames.push(name);
            newNames++;
          }
        }
      }
      workloadReports.push({
        name: workload.name,
        repeat_entry_counts: traces.map((trace) => trace.length),
        repeat_intersection: intersectionSize(repeatSets),
        repeat_union: workloadSeen.size,
        new_function_addresses: newAddresses,
        new_symbol_names: newNames,
        addresses_without_symbols: addressesWithoutSymbols,
      });
    }

    const comments = [
      "# Mach-O linker order generated from exact first function entries.",
      `# Profile: ${options.workloadProfile}; ${options.repeats} traces per workload.`,
      `# ${seenAddresses.size} function addresses; ${orderedNames.length} symbol names.`,
    ];
    await Deno.writeTextFile(
      options.output,
      [...comments, ...orderedNames, ""].join("\n"),
    );
    const report = {
      platform: "darwin",
      architecture: "aarch64",
      binary,
      order_file: options.output,
      workload_profile: options.workloadProfile,
      repeats: options.repeats,
      lc_function_starts: totalStarts,
      traced_function_addresses: seenAddresses.size,
      symbol_names: orderedNames.length,
      workloads: workloadReports,
    };
    await Deno.writeTextFile(
      `${options.output}.json`,
      `${JSON.stringify(report, null, 2)}\n`,
    );
    console.log(JSON.stringify(report, null, 2));
  } finally {
    await Deno.remove(root, { recursive: true });
  }
}

if (import.meta.main) {
  await generate(parseArgs(Deno.args));
}
