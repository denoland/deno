// Copyright 2018-2026 the Deno authors. MIT license.

type Sample = {
  group: string;
  sequence: number;
  index: number;
  startedAt: string;
  endedAt: string;
  port: number;
  requestsPerSec: number;
  latencyP99Ms: number;
  transferPerSecBytes: number;
  stdout: string;
  stderr: string;
};

type SampleOrder = {
  sequence: number;
  group: string;
};

type Stats = {
  n: number;
  mean: number;
  min: number;
  max: number;
  sd: number;
  cvPct: number;
  ci95Low: number;
  ci95High: number;
  ci95HalfWidth: number;
};

const root = new URL("../../..", import.meta.url).pathname.replace(/\/$/, "");

function parseArgs() {
  const args = [...Deno.args];
  const options: Record<string, string | boolean> = {};
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (!arg.startsWith("--")) {
      throw new Error(`unexpected positional argument: ${arg}`);
    }
    const key = arg.slice(2);
    if (key === "no-taskset" || key === "allow-short") {
      options[key] = true;
    } else {
      const value = args[++i];
      if (value == null) throw new Error(`missing value for ${arg}`);
      options[key] = value;
    }
  }

  return {
    deno: String(options.deno ?? "./target/release-lite/deno"),
    candidateDeno: options["candidate-deno"] == null
      ? null
      : String(options["candidate-deno"]),
    wrk: String(options.wrk ?? "./third_party/prebuilt/linux64/wrk"),
    outDir: String(options["out-dir"] ?? "target/node_http_throughput"),
    serverScript: String(
      options["server-script"] ??
        `${root}/tests/bench/node_http_throughput/server.mjs`,
    ),
    durationSecs: Number(options.duration ?? 30),
    warmups: Number(options.warmups ?? 2),
    samples: Number(options.samples ?? 15),
    connections: Number(options.connections ?? 128),
    threads: options.threads == null ? null : Number(options.threads),
    serverCpus: options["server-cpus"] == null
      ? null
      : String(options["server-cpus"]),
    wrkCpus: options["wrk-cpus"] == null ? null : String(options["wrk-cpus"]),
    noTaskset: options["no-taskset"] === true,
    allowShort: options["allow-short"] === true,
    targetId: String(
      options["target-id"] ??
        Deno.env.get("EXECUTION_TARGET_ID") ??
        Deno.env.get("AGED_EXECUTION_TARGET_ID") ??
        "unknown",
    ),
    order: String(options.order ?? "alternating"),
    seed: options.seed == null ? null : Number(options.seed),
    acceptanceThresholdPct: options["acceptance-threshold-pct"] == null
      ? null
      : Number(options["acceptance-threshold-pct"]),
  };
}

const options = parseArgs();
if (!options.allowShort && options.durationSecs < 10) {
  throw new Error("duration below 10s requires --allow-short");
}
if (options.samples < 1) {
  throw new Error("--samples must be at least 1");
}
if (options.warmups < 0) {
  throw new Error("--warmups must not be negative");
}
if (options.order !== "alternating" && options.order !== "random") {
  throw new Error("--order must be either 'alternating' or 'random'");
}

await Deno.mkdir(options.outDir, { recursive: true });

function resolvePath(path: string) {
  return path.startsWith("/") ? path : `${Deno.cwd()}/${path}`;
}

async function commandOutput(command: string, args: string[]) {
  const output = await new Deno.Command(command, {
    args,
    stdout: "piped",
    stderr: "piped",
  }).output();
  if (!output.success) {
    throw new Error(
      `${command} ${args.join(" ")} failed\n${
        new TextDecoder().decode(output.stderr)
      }`,
    );
  }
  return new TextDecoder().decode(output.stdout).trim();
}

async function cpuCount() {
  try {
    return Number(await commandOutput("nproc", []));
  } catch {
    return 1;
  }
}

function expandCpuList(list: string) {
  const cpus: number[] = [];
  for (const part of list.split(",")) {
    const range = part.trim().split("-");
    if (range.length === 1) {
      cpus.push(Number(range[0]));
    } else {
      const start = Number(range[0]);
      const end = Number(range[1]);
      for (let cpu = start; cpu <= end; cpu++) cpus.push(cpu);
    }
  }
  return cpus.filter((cpu) => Number.isInteger(cpu));
}

async function allowedCpuList() {
  try {
    const output = await commandOutput("taskset", ["-pc", String(Deno.pid)]);
    const affinity = output.split(":").at(-1)?.trim();
    if (affinity == null || affinity.length === 0) return null;
    const cpus = expandCpuList(affinity);
    return cpus.length === 0 ? null : cpus;
  } catch {
    return null;
  }
}

async function hasTaskset() {
  try {
    await commandOutput("taskset", ["--help"]);
    return true;
  } catch {
    try {
      await new Deno.Command("which", {
        args: ["taskset"],
        stdout: "null",
        stderr: "null",
      }).output();
      return true;
    } catch {
      return false;
    }
  }
}

function pinnedCommand(cpus: string | null, command: string, args: string[]) {
  if (cpus == null) return { command, args };
  return { command: "taskset", args: ["-c", cpus, command, ...args] };
}

const cpuTotal = await cpuCount();
const taskset = !options.noTaskset && await hasTaskset();
const allowedCpus = taskset ? await allowedCpuList() : null;
const autoPinnedCpus = allowedCpus ?? Array.from(
  { length: cpuTotal },
  (_unused, i) => i,
);
const autoServerCpus = autoPinnedCpus.length >= 6
  ? autoPinnedCpus.slice(0, 3).join(",")
  : null;
const autoWrkCpus = autoPinnedCpus.length >= 6
  ? autoPinnedCpus.slice(3, 6).join(",")
  : null;
const serverCpus = taskset ? options.serverCpus ?? autoServerCpus : null;
const wrkCpus = taskset ? options.wrkCpus ?? autoWrkCpus : null;
const threads = options.threads ??
  (wrkCpus == null ? 1 : wrkCpus.split(",").length);
const pinning = {
  enabled: serverCpus != null && wrkCpus != null,
  serverCpus,
  wrkCpus,
  allowedCpus: allowedCpus == null ? null : allowedCpus.join(","),
  reason: taskset
    ? options.serverCpus || options.wrkCpus ? "manual" : "auto"
    : "disabled",
};

async function denoVersion(deno: string) {
  return await commandOutput(resolvePath(deno), ["--version"]);
}

async function gitHead() {
  try {
    return await commandOutput("git", ["rev-parse", "HEAD"]);
  } catch {
    return "unknown";
  }
}

async function startServer(deno: string) {
  const denoPath = resolvePath(deno);
  const command = pinnedCommand(serverCpus, denoPath, [
    "run",
    "--quiet",
    "--allow-net",
    resolvePath(options.serverScript),
  ]);
  const process = new Deno.Command(command.command, {
    args: command.args,
    stdout: "piped",
    stderr: "piped",
    env: { DENO_NO_UPDATE_CHECK: "1" },
  }).spawn();

  const stdoutReader = process.stdout
    .pipeThrough(new TextDecoderStream())
    .getReader();
  const stderrPromise = process.stderr
    .pipeThrough(new TextDecoderStream())
    .getReader()
    .read()
    .then((r) => r.value ?? "");

  const deadline = Date.now() + 10000;
  let buffer = "";
  while (Date.now() < deadline) {
    const { value, done } = await stdoutReader.read();
    if (done) break;
    buffer += value;
    const line = buffer.split(/\r?\n/).find((line) =>
      line.trim().startsWith("{")
    );
    if (line != null) {
      const { port } = JSON.parse(line);
      return {
        process,
        port: Number(port),
        command,
        async stop() {
          try {
            process.kill("SIGTERM");
          } catch {
            // Process already exited.
          }
          await process.status.catch(() => undefined);
        },
      };
    }
  }

  try {
    process.kill("SIGTERM");
  } catch {
    // Process already exited.
  }
  throw new Error(`server did not report a port: ${await stderrPromise}`);
}

function parseWrk(stdout: string, stderr: string) {
  const requests = stdout.match(/Requests\/sec:\s+([0-9.]+)/);
  const transfer = stdout.match(/Transfer\/sec:\s+([0-9.]+)([KMG]?B)/);
  const p99 = stdout.match(/\n\s+99%\s+([0-9.]+)(us|ms|s)/);
  if (requests == null || transfer == null || p99 == null) {
    throw new Error(`failed to parse wrk output\n${stdout}\n${stderr}`);
  }
  const transferScale: Record<string, number> = {
    B: 1,
    KB: 1024,
    MB: 1024 * 1024,
    GB: 1024 * 1024 * 1024,
  };
  const latencyScale: Record<string, number> = { us: 0.001, ms: 1, s: 1000 };
  return {
    requestsPerSec: Number(requests[1]),
    transferPerSecBytes: Number(transfer[1]) * transferScale[transfer[2]],
    latencyP99Ms: Number(p99[1]) * latencyScale[p99[2]],
  };
}

async function runWrk(port: number) {
  const command = pinnedCommand(wrkCpus, resolvePath(options.wrk), [
    "-t",
    String(threads),
    "-c",
    String(options.connections),
    "-d",
    `${options.durationSecs}s`,
    "--latency",
    `http://127.0.0.1:${port}/`,
  ]);
  const output = await new Deno.Command(command.command, {
    args: command.args,
    stdout: "piped",
    stderr: "piped",
  }).output();
  const stdout = new TextDecoder().decode(output.stdout);
  const stderr = new TextDecoder().decode(output.stderr);
  if (!output.success) {
    throw new Error(`wrk failed\n${stdout}\n${stderr}`);
  }
  return { ...parseWrk(stdout, stderr), stdout, stderr };
}

async function warmupGroup(
  name: string,
  server: Awaited<ReturnType<typeof startServer>>,
) {
  for (let i = 1; i <= options.warmups; i++) {
    const result = await runWrk(server.port);
    console.log(
      `${name} warmup ${i}/${options.warmups}: ${
        result.requestsPerSec.toFixed(2)
      } req/s`,
    );
  }
}

function mulberry32(seed: number) {
  return () => {
    seed |= 0;
    seed = seed + 0x6D2B79F5 | 0;
    let t = Math.imul(seed ^ seed >>> 15, 1 | seed);
    t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t;
    return ((t ^ t >>> 14) >>> 0) / 4294967296;
  };
}

function makeSampleOrder(
  controlName: string,
  candidateName: string,
): SampleOrder[] {
  const order: SampleOrder[] = [];
  if (options.order === "alternating") {
    const first = options.seed == null || options.seed % 2 === 0
      ? controlName
      : candidateName;
    const second = first === controlName ? candidateName : controlName;
    for (let i = 0; i < options.samples; i++) {
      order.push({ sequence: order.length + 1, group: first });
      order.push({ sequence: order.length + 1, group: second });
    }
    return order;
  }

  for (let i = 0; i < options.samples; i++) {
    order.push({ sequence: 0, group: controlName });
    order.push({ sequence: 0, group: candidateName });
  }
  const random = mulberry32(options.seed ?? Date.now());
  for (let i = order.length - 1; i > 0; i--) {
    const j = Math.floor(random() * (i + 1));
    [order[i], order[j]] = [order[j], order[i]];
  }
  return order.map((entry, i) => ({ ...entry, sequence: i + 1 }));
}

function stats(samples: Sample[]): Stats {
  const values = samples.map((sample) => sample.requestsPerSec);
  const n = values.length;
  const mean = values.reduce((a, b) => a + b, 0) / n;
  const variance = n < 2
    ? 0
    : values.reduce((a, b) => a + (b - mean) ** 2, 0) / (n - 1);
  const sd = Math.sqrt(variance);
  const t = tCritical95(n - 1);
  const ci95HalfWidth = n < 2 ? 0 : t * sd / Math.sqrt(n);
  return {
    n,
    mean,
    min: Math.min(...values),
    max: Math.max(...values),
    sd,
    cvPct: mean === 0 ? 0 : sd / mean * 100,
    ci95Low: mean - ci95HalfWidth,
    ci95High: mean + ci95HalfWidth,
    ci95HalfWidth,
  };
}

function tCritical95(df: number) {
  if (df <= 0) return 0;
  const table = [
    12.706,
    4.303,
    3.182,
    2.776,
    2.571,
    2.447,
    2.365,
    2.306,
    2.262,
    2.228,
    2.201,
    2.179,
    2.160,
    2.145,
    2.131,
    2.120,
    2.110,
    2.101,
    2.093,
    2.086,
    2.080,
    2.074,
    2.069,
    2.064,
    2.060,
    2.056,
    2.052,
    2.048,
    2.045,
    2.042,
  ];
  if (df <= table.length) return table[df - 1];
  if (df <= 40) return 2.021;
  if (df <= 60) return 2.000;
  if (df <= 120) return 1.980;
  return 1.960;
}

function welchDelta(a: Stats, b: Stats) {
  const delta = b.mean - a.mean;
  const se = Math.sqrt(a.sd ** 2 / a.n + b.sd ** 2 / b.n);
  const numerator = (a.sd ** 2 / a.n + b.sd ** 2 / b.n) ** 2;
  const denominator = (a.sd ** 2 / a.n) ** 2 / Math.max(1, a.n - 1) +
    (b.sd ** 2 / b.n) ** 2 / Math.max(1, b.n - 1);
  const df = denominator === 0 ? 0 : numerator / denominator;
  const half = se * tCritical95(Math.round(df));
  return {
    delta,
    pct: delta / a.mean * 100,
    ci95Low: delta - half,
    ci95High: delta + half,
    ci95LowPct: (delta - half) / a.mean * 100,
    ci95HighPct: (delta + half) / a.mean * 100,
  };
}

function fmt(value: number) {
  return value.toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

const controlDeno = resolvePath(options.deno);
const candidateDeno = options.candidateDeno == null
  ? controlDeno
  : resolvePath(options.candidateDeno);
const controlName = options.candidateDeno == null ? "control_a" : "control";
const candidateName = options.candidateDeno == null ? "control_b" : "candidate";
const sampleOrder = makeSampleOrder(controlName, candidateName);
const sampleCounts: Record<string, number> = {
  [controlName]: 0,
  [candidateName]: 0,
};
const samples: Sample[] = [];

const controlServer = await startServer(controlDeno);
let candidateServer: Awaited<ReturnType<typeof startServer>> | null = null;
try {
  candidateServer = await startServer(candidateDeno);
  await warmupGroup(controlName, controlServer);
  await warmupGroup(candidateName, candidateServer);

  for (const entry of sampleOrder) {
    const server = entry.group === controlName
      ? controlServer
      : candidateServer;
    const startedAt = new Date().toISOString();
    const result = await runWrk(server.port);
    const endedAt = new Date().toISOString();
    const index = ++sampleCounts[entry.group];
    samples.push({
      group: entry.group,
      sequence: entry.sequence,
      index,
      startedAt,
      endedAt,
      port: server.port,
      ...result,
    });
    console.log(
      `${entry.group} sample ${index}/${options.samples} ` +
        `(sequence ${entry.sequence}/${sampleOrder.length}): ` +
        `${result.requestsPerSec.toFixed(2)} req/s`,
    );
  }
} finally {
  await candidateServer?.stop();
  await controlServer.stop();
}

const control = samples.filter((sample) => sample.group === controlName);
const candidate = samples.filter((sample) => sample.group === candidateName);
const controlStats = stats(control);
const candidateStats = stats(candidate);
const delta = welchDelta(controlStats, candidateStats);
const thresholdPct = options.acceptanceThresholdPct ??
  Math.max(Math.abs(delta.ci95LowPct), Math.abs(delta.ci95HighPct));

const command = [
  Deno.execPath(),
  "run",
  "--allow-read",
  "--allow-run",
  "--allow-write",
  "--allow-env",
  new URL(import.meta.url).pathname,
  ...Deno.args,
];

const result = {
  generatedAt: new Date().toISOString(),
  command,
  gitHead: await gitHead(),
  targetId: options.targetId,
  options: {
    ...options,
    deno: controlDeno,
    candidateDeno,
    wrk: resolvePath(options.wrk),
    serverScript: resolvePath(options.serverScript),
  },
  pinning,
  threads,
  sampleOrder,
  samples,
  serverCommands: {
    [controlName]: controlServer.command,
    [candidateName]: candidateServer?.command,
  },
  control: {
    name: controlName,
    deno: controlDeno,
    denoVersion: await denoVersion(controlDeno),
    samples: control,
    stats: controlStats,
  },
  candidate: {
    name: candidateName,
    deno: candidateDeno,
    denoVersion: await denoVersion(candidateDeno),
    samples: candidate,
    stats: candidateStats,
  },
  delta,
  acceptance: {
    thresholdPct,
    meanDeltaExceedsThreshold: delta.pct > thresholdPct,
    ciExcludesZero: delta.ci95Low > 0 || delta.ci95High < 0,
    clears: delta.pct > thresholdPct && delta.ci95Low > 0,
  },
};

await Deno.writeTextFile(
  `${options.outDir}/node_http_throughput.json`,
  JSON.stringify(result, null, 2) + "\n",
);

const md = `# node:http throughput benchmark

## Configuration

- Execution target id: ${options.targetId}
- Git HEAD: ${result.gitHead}
- Control Deno: ${controlDeno}
- Candidate Deno: ${candidateDeno}
- Control Deno version: ${result.control.denoVersion.replaceAll("\n", "; ")}
- Candidate Deno version: ${result.candidate.denoVersion.replaceAll("\n", "; ")}
- wrk: ${resolvePath(options.wrk)}
- Workload: ${resolvePath(options.serverScript)}
- CPU pinning: ${
  pinning.enabled ? "enabled" : "disabled"
} (${pinning.reason}); server CPUs ${pinning.serverCpus ?? "none"}; wrk CPUs ${
  pinning.wrkCpus ?? "none"
}; allowed CPUs ${pinning.allowedCpus ?? "unknown"}
- wrk settings: ${threads} threads, ${options.connections} connections, ${options.durationSecs}s duration
- Warmups: ${options.warmups} per group
- Measured samples: ${options.samples} per group
- Sample order: ${options.order}${
  options.seed == null ? "" : `; seed ${options.seed}`
}

## Benchmark Results

| group | n | mean req/s | sd | CV | 95% CI req/s |
| --- | ---: | ---: | ---: | ---: | ---: |
| ${result.control.name} | ${controlStats.n} | ${fmt(controlStats.mean)} | ${
  fmt(controlStats.sd)
} | ${fmt(controlStats.cvPct)}% | ${fmt(controlStats.ci95Low)} .. ${
  fmt(controlStats.ci95High)
} |
| ${result.candidate.name} | ${candidateStats.n} | ${
  fmt(candidateStats.mean)
} | ${fmt(candidateStats.sd)} | ${fmt(candidateStats.cvPct)}% | ${
  fmt(candidateStats.ci95Low)
} .. ${fmt(candidateStats.ci95High)} |

Candidate-vs-control delta: ${fmt(delta.delta)} req/s (${
  fmt(delta.pct)
}%), 95% CI ${fmt(delta.ci95Low)} .. ${fmt(delta.ci95High)} req/s (${
  fmt(delta.ci95LowPct)
}% .. ${fmt(delta.ci95HighPct)}%).

Acceptance threshold: ${fmt(thresholdPct)}%. Clears threshold: ${
  result.acceptance.clears ? "yes" : "no"
}.

## Sample Order

${sampleOrder.map((entry) => `${entry.sequence}. ${entry.group}`).join("\n")}

## Command

\`\`\`sh
${command.join(" ")}
\`\`\`
`;

await Deno.writeTextFile(`${options.outDir}/node_http_throughput.md`, md);
