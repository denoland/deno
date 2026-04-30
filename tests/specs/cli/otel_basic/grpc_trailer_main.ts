// Copyright 2018-2026 the Deno authors. MIT license.

// gRPC test server that sends grpc-status in HTTP/2 trailers (normal form)
// instead of initial response headers (Trailers-Only form). This exercises
// the trailer-reading code path in grpc_send.

import * as http2 from "node:http2";
import * as fs from "node:fs";

import {
  decodeExportLogsRequest,
  decodeExportMetricsRequest,
  decodeExportTraceRequest,
} from "./proto_decode.ts";

const data = {
  spans: [] as unknown[],
  logs: [] as unknown[],
  metrics: [] as unknown[],
};

function decodeGrpcBody(
  path: string,
  bytes: Uint8Array,
): Record<string, unknown> | null {
  if (path.includes("TraceService")) return decodeExportTraceRequest(bytes);
  if (path.includes("MetricsService")) return decodeExportMetricsRequest(bytes);
  if (path.includes("LogsService")) return decodeExportLogsRequest(bytes);
  return null;
}

function collectData(body: Record<string, unknown>) {
  (body.resourceLogs as { scopeLogs: { logRecords: unknown[] }[] }[] ?? [])
    .forEach((rLogs) => {
      rLogs.scopeLogs.forEach((sLogs) => {
        data.logs.push(...sLogs.logRecords);
      });
    });
  (body.resourceSpans as { scopeSpans: { spans: unknown[] }[] }[] ?? [])
    .forEach((rSpans) => {
      rSpans.scopeSpans.forEach((sSpans) => {
        data.spans.push(...sSpans.spans);
      });
    });
  (body.resourceMetrics as { scopeMetrics: { metrics: unknown[] }[] }[] ?? [])
    .forEach((rMetrics) => {
      rMetrics.scopeMetrics.forEach((sMetrics) => {
        data.metrics.push(...sMetrics.metrics);
      });
    });
}

// The grpc-status to send in trailers. "0" = OK, anything else = error.
const grpcStatus = Deno.env.get("GRPC_TRAILER_STATUS") || "0";
const grpcMessage = Deno.env.get("GRPC_TRAILER_MESSAGE") || "";

const server = http2.createSecureServer({
  key: fs.readFileSync("../../../testdata/tls/localhost.key"),
  cert: fs.readFileSync("../../../testdata/tls/localhost.crt"),
});

server.on("stream", (stream, headers) => {
  const path = headers[":path"] as string;

  const chunks: Buffer[] = [];
  stream.on("data", (chunk: Buffer) => chunks.push(chunk));
  stream.on("end", () => {
    const rawBody = Buffer.concat(chunks);
    // Strip the 5-byte gRPC frame prefix (compression flag + length)
    const msgBytes = new Uint8Array(rawBody.buffer, rawBody.byteOffset + 5);
    const body = decodeGrpcBody(path, msgBytes);
    if (body) collectData(body);

    // Send response headers WITHOUT grpc-status (normal form, not Trailers-Only)
    stream.respond(
      {
        ":status": 200,
        "content-type": "application/grpc",
      },
      { waitForTrailers: true },
    );

    stream.end();
  });

  stream.on("wantTrailers", () => {
    const trailers: Record<string, string> = {
      "grpc-status": grpcStatus,
    };
    if (grpcMessage) {
      trailers["grpc-message"] = grpcMessage;
    }
    stream.sendTrailers(trailers);
  });
});

server.listen(0, "localhost", () => {
  const addr = server.address() as { port: number };
  const port = addr.port;
  const endpoint = `https://localhost:${port}`;

  const command = new Deno.Command(Deno.execPath(), {
    args: [
      "run",
      "--env-file=env_file",
      "-A",
      "-q",
      "--unstable-cron",
      Deno.args[0],
    ],
    env: {
      OTEL_EXPORTER_OTLP_ENDPOINT: endpoint,
      OTEL_EXPORTER_OTLP_PROTOCOL: "grpc",
    },
    stdout: "null",
  });
  const child = command.spawn();
  child.status
    .then((status) => {
      if (status.signal) {
        throw new Error("child process failed: " + JSON.stringify(status));
      }
      server.close();

      data.logs.sort((a: any, b: any) =>
        Number(
          BigInt(a.observedTimeUnixNano) - BigInt(b.observedTimeUnixNano),
        )
      );
      data.spans.sort((a: any, b: any) =>
        Number(BigInt(`0x${a.spanId}`) - BigInt(`0x${b.spanId}`))
      );
      // v8js metrics are non-deterministic
      data.metrics = data.metrics.filter(
        (m: any) => !m.name.startsWith("v8js"),
      );
      data.metrics.sort((a: any, b: any) =>
        (a as any).name.localeCompare((b as any).name)
      );
      for (const metric of data.metrics) {
        const m = metric as any;
        if ("histogram" in m) {
          m.histogram.dataPoints.sort(
            (a: any, b: any) => {
              const aKey = a.attributes
                .sort((x: any, y: any) => x.key.localeCompare(y.key))
                .map(({ key, value }: any) => `${key}:${JSON.stringify(value)}`)
                .join("|");
              const bKey = b.attributes
                .sort((x: any, y: any) => x.key.localeCompare(y.key))
                .map(({ key, value }: any) => `${key}:${JSON.stringify(value)}`)
                .join("|");
              return aKey.localeCompare(bKey);
            },
          );
          for (const dataPoint of m.histogram.dataPoints) {
            dataPoint.attributes.sort((a: any, b: any) =>
              a.key.localeCompare(b.key)
            );
          }
        }
        if ("sum" in m) {
          m.sum.dataPoints.sort(
            (a: any, b: any) => {
              const aKey = a.attributes
                .sort((x: any, y: any) => x.key.localeCompare(y.key))
                .map(({ key, value }: any) => `${key}:${JSON.stringify(value)}`)
                .join("|");
              const bKey = b.attributes
                .sort((x: any, y: any) => x.key.localeCompare(y.key))
                .map(({ key, value }: any) => `${key}:${JSON.stringify(value)}`)
                .join("|");
              return aKey.localeCompare(bKey);
            },
          );
          for (const dataPoint of m.sum.dataPoints) {
            dataPoint.attributes.sort((a: any, b: any) =>
              a.key.localeCompare(b.key)
            );
          }
        }
      }
      console.log(JSON.stringify(data, null, 2));
    });
});
