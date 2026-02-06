import { parseArgs } from "jsr:@std/cli@1/parse-args";

interface Candidate {
  name: string;
  runtime: string;
  binary: string;
}

interface BenchmarkResult {
  socket: {
    throughputMiBPerSec: number;
    durationSec: number;
    sentBytes: number;
    receivedBytes: number;
  } | null;
  http: {
    requests: number;
    bytesOut: number;
    uptimeSec: number;
  } | null;
  oha: {
    summary: {
      requestsPerSec: number;
      average: number;
      slowest: number;
      fastest: number;
      total: number;
    };
  } | null;
}

function parseCandidate(spec: string): Candidate {
  // format: name=runtime@binary
  const eqIdx = spec.indexOf("=");
  if (eqIdx === -1) throw new Error(`Invalid candidate: ${spec}`);
  const name = spec.slice(0, eqIdx);
  const rest = spec.slice(eqIdx + 1);
  const atIdx = rest.indexOf("@");
  if (atIdx === -1) throw new Error(`Invalid candidate: ${spec}`);
  return {
    name,
    runtime: rest.slice(0, atIdx),
    binary: rest.slice(atIdx + 1),
  };
}

function parseCli(): {
  trials: number;
  candidates: Candidate[];
  orchestratorArgs: string[];
} {
  const argv = Deno.args;
  const candidates: Candidate[] = [];
  let trials = 3;
  let restIdx = -1;

  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--") {
      restIdx = i + 1;
      break;
    }
    if (argv[i] === "--trials" && argv[i + 1]) {
      trials = Number.parseInt(argv[i + 1], 10);
      i++;
      continue;
    }
    if (argv[i] === "--candidate" && argv[i + 1]) {
      candidates.push(parseCandidate(argv[i + 1]));
      i++;
      continue;
    }
  }

  const orchestratorArgs = restIdx >= 0 ? argv.slice(restIdx) : [];

  if (candidates.length === 0) {
    throw new Error("At least one --candidate is required");
  }

  return { trials, candidates, orchestratorArgs };
}

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[mid - 1] + sorted[mid]) / 2
    : sorted[mid];
}

async function runTrial(
  candidate: Candidate,
  orchestratorArgs: string[],
): Promise<BenchmarkResult> {
  const args = [
    "run",
    "-A",
    "benchmark-orchestrator.ts",
    "--worker-runtime",
    candidate.runtime,
    "--worker-binary",
    candidate.binary,
    ...orchestratorArgs,
  ];

  console.error(`  Running: deno ${args.join(" ")}`);

  const cmd = new Deno.Command("deno", {
    args,
    stdin: "null",
    stdout: "piped",
    stderr: "inherit",
  });
  const output = await cmd.output();

  if (!output.success) {
    throw new Error(
      `Orchestrator failed for ${candidate.name} (exit ${output.code})`,
    );
  }

  return JSON.parse(new TextDecoder().decode(output.stdout));
}

interface MedianResult {
  socketMiBPerSec: number;
  httpRps: number;
  httpLatencyAvgMs: number;
}

function computeMedians(results: BenchmarkResult[]): MedianResult {
  const socketSpeeds = results
    .filter((r) => r.socket)
    .map((r) => r.socket!.throughputMiBPerSec);

  // deno-lint-ignore no-explicit-any
  const ohaRps = results
    .filter((r) => r.oha)
    .map((r) => (r.oha as any).summary.requestsPerSec);

  // deno-lint-ignore no-explicit-any
  const ohaLatency = results
    .filter((r) => r.oha)
    .map((r) => (r.oha as any).summary.average * 1000); // sec -> ms

  return {
    socketMiBPerSec: socketSpeeds.length > 0 ? median(socketSpeeds) : 0,
    httpRps: ohaRps.length > 0 ? median(ohaRps) : 0,
    httpLatencyAvgMs: ohaLatency.length > 0 ? median(ohaLatency) : 0,
  };
}

function padRight(s: string, n: number): string {
  return s + " ".repeat(Math.max(0, n - s.length));
}

function padLeft(s: string, n: number): string {
  return " ".repeat(Math.max(0, n - s.length)) + s;
}

function printTable(rows: { name: string; medians: MedianResult }[]): void {
  const cols = [
    { header: "Runtime", width: 14 },
    { header: "Socket MiB/s", width: 14 },
    { header: "HTTP rps", width: 12 },
    { header: "Latency avg", width: 13 },
  ];

  const hLine = (left: string, mid: string, right: string, fill: string) =>
    left + cols.map((c) => fill.repeat(c.width)).join(mid) + right;

  console.log(hLine("\u250c", "\u252c", "\u2510", "\u2500"));

  const header =
    "\u2502" +
    cols
      .map((c) => " " + padRight(c.header, c.width - 2) + " ")
      .join("\u2502") +
    "\u2502";
  console.log(header);
  console.log(hLine("\u251c", "\u253c", "\u2524", "\u2500"));

  for (const row of rows) {
    const cells = [
      " " + padRight(row.name, cols[0].width - 2) + " ",
      " " + padLeft(row.medians.socketMiBPerSec.toFixed(2), cols[1].width - 2) + " ",
      " " + padLeft(Math.round(row.medians.httpRps).toString(), cols[2].width - 2) + " ",
      " " + padLeft(row.medians.httpLatencyAvgMs.toFixed(2) + "ms", cols[3].width - 2) + " ",
    ];
    console.log("\u2502" + cells.join("\u2502") + "\u2502");
  }

  console.log(hLine("\u2514", "\u2534", "\u2518", "\u2500"));
}

async function main() {
  const { trials, candidates, orchestratorArgs } = parseCli();

  console.error(
    `Running ${trials} trial(s) for ${candidates.length} candidate(s)`,
  );
  console.error(`Orchestrator args: ${orchestratorArgs.join(" ")}`);

  const allResults: { name: string; medians: MedianResult }[] = [];

  for (const candidate of candidates) {
    console.error(
      `\nCandidate: ${candidate.name} (${candidate.runtime}@${candidate.binary})`,
    );
    const trialResults: BenchmarkResult[] = [];

    for (let t = 0; t < trials; t++) {
      console.error(`  Trial ${t + 1}/${trials}`);
      const result = await runTrial(candidate, orchestratorArgs);
      trialResults.push(result);

      if (result.socket) {
        console.error(
          `    Socket: ${result.socket.throughputMiBPerSec.toFixed(2)} MiB/s`,
        );
      }
      if (result.oha) {
        // deno-lint-ignore no-explicit-any
        const summary = (result.oha as any).summary;
        console.error(
          `    HTTP: ${summary.requestsPerSec.toFixed(0)} rps, ${(summary.average * 1000).toFixed(2)}ms avg`,
        );
      }
    }

    allResults.push({
      name: candidate.name,
      medians: computeMedians(trialResults),
    });
  }

  console.log("");
  printTable(allResults);
}

main().catch((err) => {
  console.error(err);
  Deno.exit(1);
});
