import * as net from "node:net";
import * as http from "node:http";
import { Buffer } from "node:buffer";

function parseSize(value) {
  const match = /^(\d+)([kKmMgG]?)$/.exec(value);
  if (!match) {
    throw new Error(`Invalid size: ${value}`);
  }
  const n = Number.parseInt(match[1], 10);
  const unit = match[2].toLowerCase();
  const mult =
    unit === "k" ? 1024 : unit === "m" ? 1024 ** 2 : unit === "g" ? 1024 ** 3 : 1;
  return n * mult;
}

function parseArgs(argv) {
  const options = {
    host: "127.0.0.1",
    port: 9001,
    bytes: 256 * 1024 * 1024,
    chunk: 64 * 1024,
    inflight: 8 * 1024 * 1024,
    httpBind: "127.0.0.1",
    httpUrlHost: "127.0.0.1",
    httpPort: 9001,
    httpResponseBytes: 2,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const next = argv[i + 1];

    if (arg === "--host" && next) {
      options.host = next;
      i += 1;
      continue;
    }
    if (arg === "--port" && next) {
      options.port = Number.parseInt(next, 10);
      i += 1;
      continue;
    }
    if (arg === "--bytes" && next) {
      options.bytes = parseSize(next);
      i += 1;
      continue;
    }
    if (arg === "--chunk" && next) {
      options.chunk = parseSize(next);
      i += 1;
      continue;
    }
    if (arg === "--inflight" && next) {
      options.inflight = parseSize(next);
      i += 1;
      continue;
    }
    if (arg === "--http-bind" && next) {
      options.httpBind = next;
      i += 1;
      continue;
    }
    if (arg === "--http-url-host" && next) {
      options.httpUrlHost = next;
      i += 1;
      continue;
    }
    if (arg === "--http-port" && next) {
      options.httpPort = Number.parseInt(next, 10);
      i += 1;
      continue;
    }
    if (arg === "--http-response-bytes" && next) {
      options.httpResponseBytes = parseSize(next);
      i += 1;
      continue;
    }
    if (arg === "--help" || arg === "-h") {
      printUsage();
      process.exit(0);
    }

    throw new Error(`Unknown argument: ${arg}`);
  }

  if (!Number.isFinite(options.port) || options.port < 0 || options.port > 65535) {
    throw new Error(`Invalid --port: ${options.port}`);
  }
  if (
    !Number.isFinite(options.httpPort) || options.httpPort < 0 || options.httpPort > 65535
  ) {
    throw new Error(`Invalid --http-port: ${options.httpPort}`);
  }
  if (options.bytes <= 0) {
    throw new Error("--bytes must be > 0");
  }
  if (options.chunk <= 0) {
    throw new Error("--chunk must be > 0");
  }
  if (options.inflight < options.chunk) {
    throw new Error("--inflight must be >= --chunk");
  }
  if (options.httpResponseBytes <= 0) {
    throw new Error("--http-response-bytes must be > 0");
  }

  return options;
}

function printUsage() {
  console.log("Usage: <runtime> socket-http-benchmark.mjs [options]");
  console.log("");
  console.log("Socket benchmark options:");
  console.log("  --host <host>                 Target host (default: 127.0.0.1)");
  console.log("  --port <port>                 Target port (default: 3002)");
  console.log("  --bytes <n[k|m|g]>            Total bytes to send (default: 256m)");
  console.log("  --chunk <n[k|m|g]>            Write chunk size (default: 64k)");
  console.log("  --inflight <n[k|m|g]>         Max in-flight bytes (default: 8m)");
  console.log("");
  console.log("HTTP server options:");
  console.log("  --http-bind <host>            HTTP bind host (default: 127.0.0.1)");
  console.log("  --http-url-host <host>        HTTP host reported in URL (default: 127.0.0.1)");
  console.log("  --http-port <port>            HTTP port (default: 3003)");
  console.log("  --http-response-bytes <size>  Response body bytes (default: 2)");
}

function formatBytes(bytes) {
  if (bytes >= 1024 ** 3) {
    return `${(bytes / 1024 ** 3).toFixed(2)} GiB`;
  }
  if (bytes >= 1024 ** 2) {
    return `${(bytes / 1024 ** 2).toFixed(2)} MiB`;
  }
  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(2)} KiB`;
  }
  return `${bytes} B`;
}

function runSocketBenchmark(options) {
  return new Promise((resolve, reject) => {
    const socket = net.connect({ host: options.host, port: options.port });
    socket.setNoDelay(true);

    let sent = 0;
    let received = 0;
    let inFlight = 0;
    let canWrite = true;
    let startMs = 0;
    let done = false;

    const chunkBuffer = Buffer.alloc(options.chunk, 0x61);

    const finish = () => {
      if (done) return;
      done = true;

      const durationSec = (performance.now() - startMs) / 1000;
      const throughputMiBPerSec = (received / (1024 ** 2)) / durationSec;
      const throughputGibitPerSec = ((received * 8) / (1024 ** 3)) / durationSec;

      resolve({
        host: options.host,
        port: options.port,
        sentBytes: sent,
        receivedBytes: received,
        durationSec,
        throughputMiBPerSec,
        throughputGibitPerSec,
      });
      socket.end();
    };

    const maybeFinish = () => {
      if (sent === options.bytes && received === options.bytes) {
        finish();
      }
    };

    const pumpWrites = () => {
      while (canWrite && sent < options.bytes && inFlight < options.inflight) {
        const remaining = options.bytes - sent;
        const size = Math.min(remaining, options.chunk, options.inflight - inFlight);
        const payload = size === options.chunk ? chunkBuffer : chunkBuffer.subarray(0, size);

        sent += size;
        inFlight += size;
        canWrite = socket.write(payload);
      }

      maybeFinish();
    };

    socket.on("connect", () => {
      startMs = performance.now();
      pumpWrites();
    });

    socket.on("data", (data) => {
      received += data.length;
      inFlight -= data.length;
      if (inFlight < 0) {
        reject(new Error("Received more bytes than sent"));
        socket.destroy();
        return;
      }
      pumpWrites();
    });

    socket.on("drain", () => {
      canWrite = true;
      pumpWrites();
    });

    socket.on("error", (error) => {
      if (!done) {
        done = true;
        reject(error);
      }
    });

    socket.on("close", () => {
      if (!done) {
        done = true;
        reject(new Error("Socket closed before benchmark completed"));
      }
    });
  });
}

const options = parseArgs(process.argv.slice(2));
const serverStart = performance.now();
const httpStats = {
  requests: 0,
  bytesOut: 0,
};
const responseBuffer = Buffer.alloc(options.httpResponseBytes, 0x78);

const server = http.createServer((_, res) => {
  httpStats.requests += 1;
  httpStats.bytesOut += responseBuffer.byteLength;
  res.writeHead(200, {
    "content-type": "text/plain; charset=utf-8",
    "cache-control": "no-store",
    "content-length": String(responseBuffer.byteLength),
  });
  res.end(responseBuffer);
});

// Pick a random high port if port is 0, since some runtimes don't
// report the actual port from server.address() when listening on 0.
let listenPort = options.httpPort;
if (listenPort === 0) {
  listenPort = 10000 + Math.floor(Math.random() * 50000);
}
let actualHttpPort = listenPort;
server.listen(listenPort, options.httpBind, () => {
  actualHttpPort = server.address().port || listenPort;
  const readyUrl = `http://${options.httpUrlHost}:${actualHttpPort}/`;
  console.log(`HTTP_SERVER_READY ${readyUrl}`);
});

let shutdownStarted = false;
function shutdown(exitCode = 0) {
  if (shutdownStarted) return;
  shutdownStarted = true;

  server.close(() => {
    const stats = {
      bind: options.httpBind,
      port: actualHttpPort,
      requests: httpStats.requests,
      bytesOut: httpStats.bytesOut,
      uptimeSec: (performance.now() - serverStart) / 1000,
    };
    console.log(`HTTP_SERVER_STATS ${JSON.stringify(stats)}`);
    process.exit(exitCode);
  });
}

process.on("SIGINT", () => shutdown(0));
process.on("SIGTERM", () => shutdown(0));

try {
  const socketStats = await runSocketBenchmark(options);
  console.log(`host:       ${socketStats.host}:${socketStats.port}`);
  console.log(`sent:       ${formatBytes(socketStats.sentBytes)}`);
  console.log(`received:   ${formatBytes(socketStats.receivedBytes)}`);
  console.log(`duration:   ${socketStats.durationSec.toFixed(3)} s`);
  console.log(
    `throughput: ${socketStats.throughputMiBPerSec.toFixed(2)} MiB/s (${socketStats.throughputGibitPerSec.toFixed(2)} Gibit/s)`,
  );
  console.log(`SOCKET_STATS ${JSON.stringify(socketStats)}`);
} catch (error) {
  console.error(`SOCKET_BENCHMARK_ERROR ${error.message}`);
  shutdown(1);
}
