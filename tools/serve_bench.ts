#!/usr/bin/env -S deno run --allow-all
// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

type BenchCase = "hello" | "json";
type BinaryName = "A" | "B";

interface BinaryConfig {
  name: BinaryName;
  path: string;
  version: string;
}

interface RunResult {
  binary: BinaryName;
  binaryPath: string;
  binaryVersion: string;
  case: BenchCase;
  iteration: number;
  command: string[];
  url: string;
  tool: "wrk";
  requestsPerSec: number | null;
  transferPerSec: string | null;
  latency: {
    avgMs: number | null;
    stdevMs: number | null;
    maxMs: number | null;
    p50Ms: number | null;
    p75Ms: number | null;
    p90Ms: number | null;
    p99Ms: number | null;
  };
  peakMemory: {
    rssKiB: number | null;
    hwmKiB: number | null;
  };
  serverPid: number;
  stdout: string;
  stderr: string;
  loadOutput: string;
}

interface Options {
  denoA: string;
  denoB?: string;
  iterations: number;
  cases: BenchCase[];
  duration: string;
  connections: number;
  threads: number;
  out?: string;
}

const DEFAULT_OUT = "target/serve-bench/serve_bench_results.json";

function usage(): never {
  console.error(`Usage:
  tools/serve_bench.ts --deno-a target/release-lite/deno [options]

Options:
  --deno-a <path>       Baseline deno binary.
  --deno-b <path>       Candidate deno binary. When set, runs A then B each iteration.
  --iterations <n>      Iterations per case. Default: 3.
  --cases <list>        Comma-separated cases: hello,json. Default: hello,json.
  --duration <wrk d>    wrk duration, for example 5s or 30s. Default: 10s.
  --connections <n>     wrk connections. Default: 128.
  --threads <n>         wrk threads. Default: 4.
  --out <path>          JSON output path. Default: ${DEFAULT_OUT}.
  --help                Show this message.
`);
  Deno.exit(1);
}

function parseArgs(args: string[]): Options {
  const options: Options = {
    denoA: "",
    iterations: 3,
    cases: ["hello", "json"],
    duration: "10s",
    connections: 128,
    threads: 4,
    out: DEFAULT_OUT,
  };

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    const next = () => {
      const value = args[++i];
      if (!value) usage();
      return value;
    };
    switch (arg) {
      case "--deno-a":
        options.denoA = next();
        break;
      case "--deno-b":
        options.denoB = next();
        break;
      case "--iterations":
        options.iterations = Number(next());
        break;
      case "--cases":
        options.cases = next().split(",").map((caseName) => {
          if (caseName !== "hello" && caseName !== "json") {
            throw new Error(`Unknown case: ${caseName}`);
          }
          return caseName;
        });
        break;
      case "--duration":
        options.duration = next();
        break;
      case "--connections":
        options.connections = Number(next());
        break;
      case "--threads":
        options.threads = Number(next());
        break;
      case "--out":
        options.out = next();
        break;
      case "--help":
      case "-h":
        usage();
        break;
      default:
        throw new Error(`Unknown argument: ${arg}`);
    }
  }

  if (!options.denoA) usage();
  if (!Number.isInteger(options.iterations) || options.iterations < 1) {
    throw new Error("--iterations must be a positive integer");
  }
  if (!Number.isInteger(options.connections) || options.connections < 1) {
    throw new Error("--connections must be a positive integer");
  }
  if (!Number.isInteger(options.threads) || options.threads < 1) {
    throw new Error("--threads must be a positive integer");
  }
  if (options.cases.length === 0) {
    throw new Error("--cases must include at least one case");
  }

  return options;
}

async function commandOutput(
  command: string,
  args: string[],
): Promise<{ code: number; stdout: string; stderr: string }> {
  const output = await new Deno.Command(command, {
    args,
    stdout: "piped",
    stderr: "piped",
  }).output();
  const decoder = new TextDecoder();
  return {
    code: output.code,
    stdout: decoder.decode(output.stdout),
    stderr: decoder.decode(output.stderr),
  };
}

async function realPath(path: string): Promise<string> {
  try {
    const stat = await Deno.stat(path);
    if (!stat.isFile) throw new Error(`${path} is not a file`);
    return await Deno.realPath(path);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`Unable to access binary ${path}: ${message}`);
  }
}

async function binaryConfig(
  name: BinaryName,
  path: string,
): Promise<BinaryConfig> {
  const resolved = await realPath(path);
  const versionOutput = await commandOutput(resolved, ["--version"]);
  if (versionOutput.code !== 0) {
    throw new Error(
      `${resolved} --version failed:\n${versionOutput.stderr}`,
    );
  }
  return {
    name,
    path: resolved,
    version: versionOutput.stdout.trim(),
  };
}

async function requireWrk(): Promise<string> {
  const result = await commandOutput("sh", ["-c", "command -v wrk"]);
  if (result.code !== 0 || result.stdout.trim() === "") {
    throw new Error(
      "wrk was not found on PATH. Install wrk or extend this harness with an available load tool before measuring.",
    );
  }
  return result.stdout.trim();
}

async function writeServerScript(path: string): Promise<void> {
  await Deno.mkdir(dirname(path), { recursive: true });
  await Deno.writeTextFile(
    path,
    `// Generated by tools/serve_bench.ts.
const port = Number(Deno.env.get("SERVE_BENCH_PORT"));
const hostname = "127.0.0.1";
const benchCase = Deno.env.get("SERVE_BENCH_CASE") ?? "hello";
const textHeaders = new Headers({ "content-type": "text/plain" });
const jsonHeaders = new Headers({
  "content-type": "application/json",
  "cache-control": "public, max-age=60",
});

function hello() {
  return new Response("Hello World\\n", { headers: textHeaders });
}

function json(req) {
  const url = new URL(req.url);
  const id = url.searchParams.get("id") ?? url.pathname.split("/").at(-1);
  const userAgent = req.headers.get("user-agent") ?? "";
  const payload = {
    ok: true,
    id,
    method: req.method,
    path: url.pathname,
    include: url.searchParams.get("include"),
    userAgentLength: userAgent.length,
    items: [
      { id: 1, name: "alpha", active: true },
      { id: 2, name: "bravo", active: false },
      { id: 3, name: "charlie", active: true },
    ],
  };
  return new Response(JSON.stringify(payload), { headers: jsonHeaders });
}

Deno.serve({
  hostname,
  port,
  onListen(addr) {
    console.log(JSON.stringify({ ready: true, addr, benchCase }));
  },
}, benchCase === "json" ? json : hello);
`,
  );
}

function dirname(path: string): string {
  const index = path.lastIndexOf("/");
  return index === -1 ? "." : path.slice(0, index);
}

async function freePort(): Promise<number> {
  const listener = Deno.listen({ hostname: "127.0.0.1", port: 0 });
  const port = listener.addr.transport === "tcp" ? listener.addr.port : 0;
  listener.close();
  return port;
}

function streamToString(stream: ReadableStream<Uint8Array>): Promise<string> {
  return new Response(stream).text();
}

async function waitForServer(
  url: string,
  child: Deno.ChildProcess,
): Promise<void> {
  const deadline = Date.now() + 10_000;
  let lastError: unknown;
  while (Date.now() < deadline) {
    const status = await Promise.race([
      child.status.then((status: Deno.CommandStatus) => ({
        exited: true,
        status,
      })),
      Promise.resolve({ exited: false as const }),
    ]);
    if (status.exited) {
      throw new Error(`server exited before accepting requests`);
    }
    try {
      const response = await fetch(url);
      await response.body?.cancel();
      if (response.ok) return;
    } catch (error) {
      lastError = error;
    }
    await delay(50);
  }
  throw new Error(`Timed out waiting for ${url}: ${lastError}`);
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function parseMemoryStatus(status: string): {
  rssKiB: number | null;
  hwmKiB: number | null;
} {
  const rss = status.match(/^VmRSS:\s*(\d+)\s*kB\s*$/im);
  const hwm = status.match(/^VmHWM:\s*(\d+)\s*kB\s*$/im);
  return {
    rssKiB: rss ? Number(rss[1]) : null,
    hwmKiB: hwm ? Number(hwm[1]) : null,
  };
}

async function monitorMemory(
  pid: number,
  done: Promise<unknown>,
): Promise<{ rssKiB: number | null; hwmKiB: number | null }> {
  let peakRss: number | null = null;
  let peakHwm: number | null = null;
  let stopped = false;
  done.finally(() => {
    stopped = true;
  });

  while (true) {
    try {
      const status = await Deno.readTextFile(`/proc/${pid}/status`);
      const memory = parseMemoryStatus(status);
      if (memory.rssKiB !== null) {
        peakRss = Math.max(peakRss ?? 0, memory.rssKiB);
      }
      if (memory.hwmKiB !== null) {
        peakHwm = Math.max(peakHwm ?? 0, memory.hwmKiB);
      }
    } catch {
      break;
    }
    if (stopped) break;
    await delay(50);
  }

  return { rssKiB: peakRss, hwmKiB: peakHwm };
}

function parseDuration(value: string): number {
  const match = value.match(/^([0-9.]+)(ms|s|m|h)?$/);
  if (!match) return Number.NaN;
  const amount = Number(match[1]);
  const unit = match[2] ?? "s";
  switch (unit) {
    case "ms":
      return amount;
    case "s":
      return amount * 1000;
    case "m":
      return amount * 60_000;
    case "h":
      return amount * 3_600_000;
  }
  return Number.NaN;
}

function valueToMs(value: string, unit: string): number {
  switch (unit) {
    case "us":
      return Number(value) / 1000;
    case "ms":
      return Number(value);
    case "s":
      return Number(value) * 1000;
    default:
      throw new Error(`Unknown latency unit: ${unit}`);
  }
}

function parseLatencyValue(line: string): number | null {
  const match = line.match(/([0-9.]+)(us|ms|s)/);
  return match ? valueToMs(match[1], match[2]) : null;
}

function parseWrkOutput(output: string): Pick<
  RunResult,
  "requestsPerSec" | "transferPerSec" | "latency"
> {
  const requests = output.match(/^Requests\/sec:\s+([0-9.]+)$/m);
  const transfer = output.match(/^Transfer\/sec:\s+(.+)$/m);
  const latency = output.match(
    /^\s+Latency\s+([0-9.]+)(us|ms|s)\s+([0-9.]+)(us|ms|s)\s+([0-9.]+)(us|ms|s)/m,
  );

  return {
    requestsPerSec: requests ? Number(requests[1]) : null,
    transferPerSec: transfer ? transfer[1].trim() : null,
    latency: {
      avgMs: latency ? valueToMs(latency[1], latency[2]) : null,
      stdevMs: latency ? valueToMs(latency[3], latency[4]) : null,
      maxMs: latency ? valueToMs(latency[5], latency[6]) : null,
      p50Ms: parseLatencyValue(output.match(/^\s+50%\s+(.+)$/m)?.[1] ?? ""),
      p75Ms: parseLatencyValue(output.match(/^\s+75%\s+(.+)$/m)?.[1] ?? ""),
      p90Ms: parseLatencyValue(output.match(/^\s+90%\s+(.+)$/m)?.[1] ?? ""),
      p99Ms: parseLatencyValue(output.match(/^\s+99%\s+(.+)$/m)?.[1] ?? ""),
    },
  };
}

function casePath(benchCase: BenchCase): string {
  switch (benchCase) {
    case "hello":
      return "/";
    case "json":
      return "/users/123?id=123&include=posts";
  }
}

async function stopChild(
  child: Deno.ChildProcess,
  statusPromise: Promise<Deno.CommandStatus>,
): Promise<void> {
  try {
    child.kill("SIGTERM");
  } catch {
    return;
  }
  const status = await Promise.race([
    statusPromise.then(() => "done"),
    delay(2_000).then(() => "timeout"),
  ]);
  if (status === "timeout") {
    try {
      child.kill("SIGKILL");
    } catch {
      // Already gone.
    }
    await statusPromise.catch(() => undefined);
  }
}

async function runOne(
  wrk: string,
  serverScript: string,
  options: Options,
  binary: BinaryConfig,
  benchCase: BenchCase,
  iteration: number,
): Promise<RunResult> {
  const port = await freePort();
  const url = `http://127.0.0.1:${port}${casePath(benchCase)}`;
  const server = new Deno.Command(binary.path, {
    args: ["run", "--allow-env", "--allow-net", serverScript],
    env: {
      SERVE_BENCH_PORT: String(port),
      SERVE_BENCH_CASE: benchCase,
    },
    stdout: "piped",
    stderr: "piped",
  }).spawn();
  const statusPromise = server.status;
  const stdoutPromise = streamToString(server.stdout);
  const stderrPromise = streamToString(server.stderr);

  try {
    await waitForServer(url, server);
    const command = [
      wrk,
      `-t${options.threads}`,
      `-c${options.connections}`,
      `-d${options.duration}`,
      "--latency",
      url,
    ];
    const load = new Deno.Command(command[0], {
      args: command.slice(1),
      stdout: "piped",
      stderr: "piped",
    }).output();
    const memory = await monitorMemory(server.pid, load);
    const loadOutput = await load;
    const decoder = new TextDecoder();
    const stdout = decoder.decode(loadOutput.stdout);
    const stderr = decoder.decode(loadOutput.stderr);
    if (loadOutput.code !== 0) {
      throw new Error(`wrk failed:\n${stdout}\n${stderr}`);
    }
    const parsed = parseWrkOutput(stdout);
    await stopChild(server, statusPromise);
    return {
      binary: binary.name,
      binaryPath: binary.path,
      binaryVersion: binary.version,
      case: benchCase,
      iteration,
      command,
      url,
      tool: "wrk",
      ...parsed,
      peakMemory: memory,
      serverPid: server.pid,
      stdout: await stdoutPromise,
      stderr: await stderrPromise,
      loadOutput: stdout,
    };
  } catch (error) {
    await stopChild(server, statusPromise);
    await stdoutPromise.catch(() => "");
    await stderrPromise.catch(() => "");
    throw error;
  }
}

function mean(values: number[]): number | null {
  if (values.length === 0) return null;
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function stdev(values: number[]): number | null {
  if (values.length < 2) return null;
  const avg = mean(values)!;
  return Math.sqrt(
    values.reduce((sum, value) => sum + (value - avg) ** 2, 0) /
      (values.length - 1),
  );
}

function fmt(value: number | null, fractionDigits = 2): string {
  return value === null ? "n/a" : value.toFixed(fractionDigits);
}

function summarize(results: RunResult[]): void {
  console.log("\nSummary");
  console.log(
    "| case | binary | n | req/s mean | req/s stdev | p50 ms mean | p99 ms mean | peak HWM MiB max |",
  );
  console.log(
    "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |",
  );
  const keys = new Set(
    results.map((result) => `${result.case}:${result.binary}`),
  );
  for (const key of keys) {
    const [benchCase, binary] = key.split(":");
    const subset = results.filter((result) =>
      result.case === benchCase && result.binary === binary
    );
    const rps = subset
      .map((result) => result.requestsPerSec)
      .filter((value): value is number => value !== null);
    const p50 = subset
      .map((result) => result.latency.p50Ms)
      .filter((value): value is number => value !== null);
    const p99 = subset
      .map((result) => result.latency.p99Ms)
      .filter((value): value is number => value !== null);
    const hwm = subset
      .map((result) => result.peakMemory.hwmKiB)
      .filter((value): value is number => value !== null);
    console.log(
      `| ${benchCase} | ${binary} | ${subset.length} | ${fmt(mean(rps))} | ${
        fmt(stdev(rps))
      } | ${fmt(mean(p50), 3)} | ${fmt(mean(p99), 3)} | ${
        fmt(hwm.length ? Math.max(...hwm) / 1024 : null, 1)
      } |`,
    );
  }
}

async function main() {
  const options = parseArgs(Deno.args);
  const durationMs = parseDuration(options.duration);
  if (!Number.isFinite(durationMs) || durationMs <= 0) {
    throw new Error(`Invalid --duration: ${options.duration}`);
  }

  const wrk = await requireWrk();
  const binaries = [await binaryConfig("A", options.denoA)];
  if (options.denoB) {
    binaries.push(await binaryConfig("B", options.denoB));
  }

  const serverScript = await Deno.realPath("target/serve-bench").catch(
    async () => {
      await Deno.mkdir("target/serve-bench", { recursive: true });
      return await Deno.realPath("target/serve-bench");
    },
  ).then((dir) => `${dir}/serve_bench_server.ts`);
  await writeServerScript(serverScript);

  const results: RunResult[] = [];
  for (const benchCase of options.cases) {
    for (let iteration = 1; iteration <= options.iterations; iteration++) {
      for (const binary of binaries) {
        console.log(
          `Running case=${benchCase} iteration=${iteration} binary=${binary.name}`,
        );
        const result = await runOne(
          wrk,
          serverScript,
          options,
          binary,
          benchCase,
          iteration,
        );
        console.log(
          `  ${fmt(result.requestsPerSec)} req/s, p50 ${
            fmt(result.latency.p50Ms, 3)
          } ms, p99 ${fmt(result.latency.p99Ms, 3)} ms, peak HWM ${
            fmt(
              result.peakMemory.hwmKiB === null
                ? null
                : result.peakMemory.hwmKiB / 1024,
              1,
            )
          } MiB`,
        );
        results.push(result);
      }
    }
  }

  summarize(results);

  if (options.out) {
    await Deno.mkdir(dirname(options.out), { recursive: true });
    await Deno.writeTextFile(
      options.out,
      JSON.stringify(
        {
          createdAt: new Date().toISOString(),
          options,
          binaries,
          results,
        },
        null,
        2,
      ) + "\n",
    );
    console.log(`\nWrote ${options.out}`);
  }
}

if (import.meta.main) {
  await main();
}
