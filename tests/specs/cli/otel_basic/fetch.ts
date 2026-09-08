import { trace } from "npm:@opentelemetry/api@1.9.0";

async function request(url: string, options: any) {
  try {
    await (await fetch(url, options)).text();
  } catch {
  }
}

await request("http://localhost:4545/echo.ts");
await request("http://localhost:4545/not-found");
await request("http://unreachable-host.abc/");
await request("http://localhost:4545/echo.ts", { signal: AbortSignal.abort() });

// `fetch()` must not leave its span in the ambient context after it returns:
// this span is created at the top level, so it must be a root span.
trace.getTracer("example-tracer").startActiveSpan(
  "after fetch",
  (span) => span.end(),
);
