// Copyright 2018-2026 the Deno authors. MIT license.

// Example: OpenTelemetry console exporter demo
//
// Run with:
//   OTEL_DENO=true OTEL_EXPORTER_OTLP_PROTOCOL=console ./target/debug/deno run -A tests/specs/cli/otel_console/example.ts
//
// This demonstrates spans, logs, and metrics being printed to stderr
// in human-readable format — no collector needed.

import { metrics, trace } from "npm:@opentelemetry/api@1.9.0";

const tracer = trace.getTracer("my-service");
const meter = metrics.getMeter("my-service");

// --- Metrics ---
const requestCounter = meter.createCounter("http.requests", {
  description: "Total HTTP requests",
});
const requestDuration = meter.createHistogram("http.request.duration", {
  description: "HTTP request duration",
  unit: "ms",
});

// --- HTTP server with tracing + metrics ---
const server = Deno.serve({
  port: 8125,
  onListen({ port }) {
    console.log(`Server listening on http://localhost:${port}`);
    // Make a few requests, then shut down
    (async () => {
      for (const path of ["/api/users", "/api/posts", "/healthz"]) {
        await fetch(`http://localhost:${port}${path}`);
      }
      server.shutdown();
    })();
  },
  handler: async (req) => {
    const url = new URL(req.url);

    return await tracer.startActiveSpan(
      `${req.method} ${url.pathname}`,
      async (span) => {
        const start = performance.now();

        span.setAttribute("http.method", req.method);
        span.setAttribute("http.url", url.pathname);

        // Simulate some async work
        await new Promise((r) => setTimeout(r, Math.random() * 50));

        // Nested span
        await tracer.startActiveSpan("db.query", async (dbSpan) => {
          dbSpan.setAttribute("db.system", "postgresql");
          dbSpan.setAttribute("db.statement", "SELECT * FROM users");
          await new Promise((r) => setTimeout(r, Math.random() * 20));
          dbSpan.end();
        });

        const duration = performance.now() - start;
        requestCounter.add(1, { method: req.method, path: url.pathname });
        requestDuration.record(duration, {
          method: req.method,
          path: url.pathname,
        });

        span.setAttribute("http.status_code", 200);
        span.end();

        return new Response(
          JSON.stringify({ path: url.pathname, duration_ms: duration }),
          { headers: { "content-type": "application/json" } },
        );
      },
    );
  },
});
