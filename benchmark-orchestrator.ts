import { parseArgs } from "jsr:@std/cli@1/parse-args";
import { TextLineStream } from "jsr:@std/streams@1/text-line-stream";

interface OrchestratorOptions {
  bytes: string;
  ohaDuration: string;
  workerRuntime: string;
  workerBinary: string;
  chunk?: string;
  inflight?: string;
  httpResponseBytes?: string;
}

function parseOptions(): OrchestratorOptions {
  const args = parseArgs(Deno.args, {
    string: [
      "bytes",
      "oha-duration",
      "worker-runtime",
      "worker-binary",
      "chunk",
      "inflight",
      "http-response-bytes",
    ],
    default: {
      bytes: "20g",
      "oha-duration": "10s",
      "worker-runtime": "deno",
      "worker-binary": "deno",
    },
  });

  return {
    bytes: args.bytes!,
    ohaDuration: args["oha-duration"]!,
    workerRuntime: args["worker-runtime"]!,
    workerBinary: args["worker-binary"]!,
    chunk: args.chunk,
    inflight: args.inflight,
    httpResponseBytes: args["http-response-bytes"],
  };
}

async function waitForMarker(
  lines: ReadableStream<string>,
  marker: string,
): Promise<{ value: string; rest: ReadableStream<string> }> {
  const reader = lines.getReader();
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) throw new Error(`Stream ended before emitting ${marker}`);
      if (value.startsWith(marker)) {
        reader.releaseLock();
        return { value: value.slice(marker.length).trim(), rest: lines };
      }
    }
  } catch (e) {
    reader.releaseLock();
    throw e;
  }
}

function stdoutLines(proc: Deno.ChildProcess): ReadableStream<string> {
  return proc.stdout
    .pipeThrough(new TextDecoderStream())
    .pipeThrough(new TextLineStream());
}

async function collectRemaining(lines: ReadableStream<string>): Promise<string[]> {
  const result: string[] = [];
  for await (const line of lines) {
    result.push(line);
  }
  return result;
}

async function run() {
  const opts = parseOptions();

  // 1. Spawn echo server
  const echoCmd = new Deno.Command("deno", {
    args: ["run", "-A", "echo-server.mjs", "--port", "0"],
    stdin: "null",
    stdout: "piped",
    stderr: "inherit",
  });
  const echoProc = echoCmd.spawn();
  const echoLines = stdoutLines(echoProc);

  const { value: echoPort } = await waitForMarker(echoLines, "ECHO_SERVER_READY ");
  console.error(`Echo server ready on port ${echoPort}`);

  // 2. Spawn worker
  const workerArgs: string[] = [];
  if (opts.workerRuntime === "deno") {
    workerArgs.push("run", "-A");
  }
  workerArgs.push(
    "socket-http-benchmark.mjs",
    "--port",
    echoPort,
    "--bytes",
    opts.bytes,
    "--http-port",
    "0",
  );
  if (opts.chunk) workerArgs.push("--chunk", opts.chunk);
  if (opts.inflight) workerArgs.push("--inflight", opts.inflight);
  if (opts.httpResponseBytes) workerArgs.push("--http-response-bytes", opts.httpResponseBytes);

  const workerCmd = new Deno.Command(opts.workerBinary, {
    args: workerArgs,
    stdin: "null",
    stdout: "piped",
    stderr: "inherit",
  });
  const workerProc = workerCmd.spawn();
  const workerLines = stdoutLines(workerProc);

  // 3. Parse HTTP_SERVER_READY from worker
  const { value: httpUrl, rest: workerRest } = await waitForMarker(
    workerLines,
    "HTTP_SERVER_READY ",
  );
  console.error(`Worker HTTP server ready at ${httpUrl}`);

  // Start collecting remaining worker lines in the background
  const remainingLinesPromise = collectRemaining(workerRest);

  // 4. Spawn oha
  console.error(`Running oha for ${opts.ohaDuration} against ${httpUrl}`);
  const ohaCmd = new Deno.Command("oha", {
    args: ["-z", opts.ohaDuration, "--no-tui", "--output-format", "json", httpUrl],
    stdin: "null",
    stdout: "piped",
    stderr: "piped",
  });
  const ohaOutput = await ohaCmd.output();

  if (!ohaOutput.success) {
    const stderr = new TextDecoder().decode(ohaOutput.stderr);
    console.error(`oha stderr: ${stderr}`);
    throw new Error(`oha exited with code ${ohaOutput.code}`);
  }

  let ohaJson: unknown;
  try {
    ohaJson = JSON.parse(new TextDecoder().decode(ohaOutput.stdout));
  } catch {
    const stdout = new TextDecoder().decode(ohaOutput.stdout);
    console.error("oha stdout:", stdout.slice(0, 500));
    throw new Error("Failed to parse oha JSON output");
  }

  // 5. Send SIGTERM to worker so it shuts down and prints HTTP_SERVER_STATS
  workerProc.kill("SIGTERM");
  await workerProc.status;

  // 6. Parse SOCKET_STATS and HTTP_SERVER_STATS from collected lines
  const allWorkerLines = await remainingLinesPromise;
  let socketStats: unknown = null;
  let httpServerStats: unknown = null;

  for (const line of allWorkerLines) {
    if (line.startsWith("SOCKET_STATS ")) {
      socketStats = JSON.parse(line.slice("SOCKET_STATS ".length));
    }
    if (line.startsWith("HTTP_SERVER_STATS ")) {
      httpServerStats = JSON.parse(line.slice("HTTP_SERVER_STATS ".length));
    }
  }

  // 7. Kill echo server
  echoProc.kill("SIGTERM");
  // Don't await - just let it die
  echoProc.status.catch(() => {});

  // 8. Output combined JSON
  const result = {
    socket: socketStats,
    http: httpServerStats,
    oha: ohaJson,
  };

  console.log(JSON.stringify(result, null, 2));
}

run().catch((err) => {
  console.error(err);
  Deno.exit(1);
});
