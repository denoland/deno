// Copyright 2018-2025 the Deno authors. MIT license.

import grpc from "npm:@grpc/grpc-js";
import protoLoader from "npm:@grpc/proto-loader";

const data = {
  spans: [],
  logs: [],
  metrics: [],
};

// Download and load OTLP proto definitions (traces, metrics, logs) from GitHub if not present
// Try to match with what's cloned into opentelemetry-proto crate
const opentelemetryProtoTag = "1.3.2";
async function ensureProtoFiles() {
  if (await fileExists("./opentelemetry-proto")) return;
  console.log("Downloading OpenTelemetry proto repo...");
  const repo = "open-telemetry/opentelemetry-proto";
  const url =
    `https://github.com/${repo}/archive/refs/tags/v${opentelemetryProtoTag}.zip`;
  const zipPath = "opentelemetry-proto.zip";
  const unzipDir = "opentelemetry-proto";
  // Download zip
  const resp = await fetch(url);
  if (!resp.ok) throw new Error(`Failed to download proto zip: ${resp.status}`);
  const zipData = new Uint8Array(await resp.arrayBuffer());
  await Deno.writeFile(zipPath, zipData);
  // Unzip
  const p = Deno.run({ cmd: ["unzip", "-q", zipPath, "-d", unzipDir] });
  const status = await p.status();
  if (!status.success) throw new Error("Failed to unzip proto files");
  // Clean up
  await Deno.remove(zipPath);
}

async function fileExists(path) {
  try {
    await Deno.stat(path);
    return true;
  } catch (_) {
    return false;
  }
}

function handleOtlpRequest(call, callback, type) {
  // call.request is the OTLP protobuf message
  // For test purposes, just push to data and return success
  console.log(
    `[grpc] Received ${type} export request`,
    JSON.stringify(call.request),
  );
  if (type === "traces") {
    call.request.resourceSpans?.forEach((rSpans) => {
      rSpans.scopeSpans.forEach((sSpans) => {
        data.spans.push(...sSpans.spans);
      });
    });
  } else if (type === "metrics") {
    call.request.resourceMetrics?.forEach((rMetrics) => {
      rMetrics.scopeMetrics.forEach((sMetrics) => {
        data.metrics.push(...sMetrics.metrics);
      });
    });
  } else if (type === "logs") {
    call.request.resourceLogs?.forEach((rLogs) => {
      rLogs.scopeLogs.forEach((sLogs) => {
        data.logs.push(...sLogs.logRecords);
      });
    });
  }
  callback(null, { partialSuccess: {} });
}

async function startGrpcServer(port, onReady) {
  // Ensure proto files are present before loading
  let otlp = await ensureProtoFiles().then(() => {
    const packageDefinition = protoLoader.loadSync([
      `opentelemetry/proto/collector/trace/v1/trace_service.proto`,
      `opentelemetry/proto/collector/metrics/v1/metrics_service.proto`,
      `opentelemetry/proto/collector/logs/v1/logs_service.proto`,
    ], {
      includeDirs: [
        `./opentelemetry-proto/opentelemetry-proto-${opentelemetryProtoTag}`,
      ],
      keepCase: true,
      longs: String,
      enums: String,
      defaults: true,
      oneofs: true,
    });
    return grpc.loadPackageDefinition(packageDefinition).opentelemetry.proto;
  });
  const server = new grpc.Server();
  // Register minimal OTLP services
  server.addService(otlp.collector.trace.v1.TraceService.service, {
    Export: (call, callback) => handleOtlpRequest(call, callback, "traces"),
  });
  server.addService(otlp.collector.metrics.v1.MetricsService.service, {
    Export: (call, callback) => handleOtlpRequest(call, callback, "metrics"),
  });
  server.addService(otlp.collector.logs.v1.LogsService.service, {
    Export: (call, callback) => handleOtlpRequest(call, callback, "logs"),
  });
  /// Read TLS key and cert files (same as HTTP server)
  // const key = Deno.readTextFileSync("../../../testdata/tls/localhost.key");
  // const cert = Deno.readTextFileSync("../../../testdata/tls/localhost.crt");
  // const keyBuf = Buffer.from(key);
  // const certBuf = Buffer.from(cert);
  // const creds = grpc.ServerCredentials.createSsl(
  //   null,
  //   [{ private_key: keyBuf, cert_chain: certBuf }],
  //   true,
  // );
  /// Error: Not implemented: http2.createSecureServer
  /// Use insecure credentials for gRPC in Deno (TLS not supported)
  const creds = grpc.ServerCredentials.createInsecure();
  server.bindAsync(
    `0.0.0.0:${port}`,
    creds,
    (err, actualPort) => {
      if (err) throw err;
      console.log(`[grpc] Server listening on port ${actualPort}`);
      onReady(actualPort, server);
    },
  );
}

async function handler(req) {
  const body = await req.json();
  body.resourceLogs?.forEach((rLogs) => {
    rLogs.scopeLogs.forEach((sLogs) => {
      data.logs.push(...sLogs.logRecords);
    });
  });
  body.resourceSpans?.forEach((rSpans) => {
    rSpans.scopeSpans.forEach((sSpans) => {
      data.spans.push(...sSpans.spans);
    });
  });
  body.resourceMetrics?.forEach((rMetrics) => {
    rMetrics.scopeMetrics.forEach((sMetrics) => {
      data.metrics.push(...sMetrics.metrics);
    });
  });
  return Response.json({ partialSuccess: {} }, { status: 200 });
}

const protocol = Deno.env.get("OTEL_EXPORTER_OTLP_PROTOCOL")?.toLowerCase();

// Only run the necessary collector server
switch (protocol) {
  case "grpc": {
    startGrpcServer(0, async (port, server) => {
      const command = new Deno.Command(Deno.execPath(), {
        args: [
          "run",
          "--env-file=env_file",
          "-A",
          "-q",
          Deno.args[0],
        ],
        env: {
          // rest of env is in env_file
          OTEL_EXPORTER_OTLP_ENDPOINT: `http://localhost:${port}`,
        },
        stdout: "null",
      });
      const child = command.spawn();
      child.status
        .then((status) => {
          if (status.signal) {
            throw new Error("child process failed: " + JSON.stringify(status));
          }
          server.tryShutdown(() => {});
        })
        .then(() => {
          data.logs.sort((a, b) =>
            Number(
              BigInt(a.observedTimeUnixNano) - BigInt(b.observedTimeUnixNano),
            )
          );
          data.spans.sort((a, b) =>
            Number(BigInt(`0x${a.spanId}`) - BigInt(`0x${b.spanId}`))
          );
          // v8js metrics are non-deterministic
          data.metrics = data.metrics.filter((m) => !m.name.startsWith("v8js"));
          data.metrics.sort((a, b) => a.name.localeCompare(b.name));
          for (const metric of data.metrics) {
            if ("histogram" in metric) {
              metric.histogram.dataPoints.sort((a, b) => {
                const aKey = a.attributes
                  .sort((x, y) => x.key.localeCompare(y.key))
                  .map(({ key, value }) => `${key}:${JSON.stringify(value)}`)
                  .join("|");
                const bKey = b.attributes
                  .sort((x, y) => x.key.localeCompare(y.key))
                  .map(({ key, value }) => `${key}:${JSON.stringify(value)}`)
                  .join("|");
                return aKey.localeCompare(bKey);
              });

              for (const dataPoint of metric.histogram.dataPoints) {
                dataPoint.attributes.sort((a, b) => {
                  return a.key.localeCompare(b.key);
                });
              }
            }
            if ("sum" in metric) {
              metric.sum.dataPoints.sort((a, b) => {
                const aKey = a.attributes
                  .sort((x, y) => x.key.localeCompare(y.key))
                  .map(({ key, value }) => `${key}:${JSON.stringify(value)}`)
                  .join("|");
                const bKey = b.attributes
                  .sort((x, y) => x.key.localeCompare(y.key))
                  .map(({ key, value }) => `${key}:${JSON.stringify(value)}`)
                  .join("|");
                return aKey.localeCompare(bKey);
              });

              for (const dataPoint of metric.sum.dataPoints) {
                dataPoint.attributes.sort((a, b) => {
                  return a.key.localeCompare(b.key);
                });
              }
            }
          }
          console.log(JSON.stringify(data, null, 2));
        });
    });
    break;
  }
  case "http/protobuf":
  case "http/json": {
    let server;
    function onListen({ port }) {
      const command = new Deno.Command(Deno.execPath(), {
        args: [
          "run",
          "--env-file=env_file",
          "-A",
          "-q",
          Deno.args[0],
        ],
        env: {
          OTEL_EXPORTER_OTLP_ENDPOINT: `https://localhost:${port}`,
        },
        stdout: "null",
      });
      const child = command.spawn();
      child.status
        .then((status) => {
          if (status.signal) {
            throw new Error("child process failed: " + JSON.stringify(status));
          }
          server.shutdown();
        })
        .then(() => {
          data.logs.sort((a, b) =>
            Number(
              BigInt(a.observedTimeUnixNano) - BigInt(b.observedTimeUnixNano),
            )
          );
          data.spans.sort((a, b) =>
            Number(BigInt(`0x${a.spanId}`) - BigInt(`0x${b.spanId}`))
          );
          // v8js metrics are non-deterministic
          data.metrics = data.metrics.filter((m) => !m.name.startsWith("v8js"));
          data.metrics.sort((a, b) => a.name.localeCompare(b.name));
          for (const metric of data.metrics) {
            if ("histogram" in metric) {
              metric.histogram.dataPoints.sort((a, b) => {
                const aKey = a.attributes
                  .sort((x, y) => x.key.localeCompare(y.key))
                  .map(({ key, value }) => `${key}:${JSON.stringify(value)}`)
                  .join("|");
                const bKey = b.attributes
                  .sort((x, y) => x.key.localeCompare(y.key))
                  .map(({ key, value }) => `${key}:${JSON.stringify(value)}`)
                  .join("|");
                return aKey.localeCompare(bKey);
              });

              for (const dataPoint of metric.histogram.dataPoints) {
                dataPoint.attributes.sort((a, b) => {
                  return a.key.localeCompare(b.key);
                });
              }
            }
            if ("sum" in metric) {
              metric.sum.dataPoints.sort((a, b) => {
                const aKey = a.attributes
                  .sort((x, y) => x.key.localeCompare(y.key))
                  .map(({ key, value }) => `${key}:${JSON.stringify(value)}`)
                  .join("|");
                const bKey = b.attributes
                  .sort((x, y) => x.key.localeCompare(y.key))
                  .map(({ key, value }) => `${key}:${JSON.stringify(value)}`)
                  .join("|");
                return aKey.localeCompare(bKey);
              });

              for (const dataPoint of metric.sum.dataPoints) {
                dataPoint.attributes.sort((a, b) => {
                  return a.key.localeCompare(b.key);
                });
              }
            }
          }
          console.log(JSON.stringify(data, null, 2));
        });
    }
    if (Deno.env.get("OTEL_DENO_VSOCK")) {
      server = Deno.serve({
        cid: -1,
        port: 4317,
        onListen,
        handler,
      });
    } else {
      server = Deno.serve({
        key: Deno.readTextFileSync("../../../testdata/tls/localhost.key"),
        cert: Deno.readTextFileSync("../../../testdata/tls/localhost.crt"),
        port: 0,
        onListen,
        handler,
      });
    }
    break;
  }
  default:
    throw new Error(`Unknown OTEL_EXPORTER_OTLP_PROTOCOL: ${protocol}`);
}
