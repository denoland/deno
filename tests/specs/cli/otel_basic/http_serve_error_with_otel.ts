// Regression test for https://panic.deno.com/v2.7.8/aarch64-apple-darwin/g7y2Non_5uBwt-hvBwk-hvB4j-hvBow9zC434tWoq3tWw32tW
// When both tracing and metrics are enabled and the onError handler throws,
// op_http_metric_handle_otel_error must not consume the external pointer
// before op_http_copy_span_to_otel_info can use it.

const server = Deno.serve({
  port: 0,
  async onListen({ port }) {
    try {
      await (await fetch(`http://localhost:${port}/`)).text();
    } finally {
      server.shutdown();
    }
  },
  handler: (_req) => {
    throw new Error("handler error");
  },
  onError: (_error) => {
    // onError itself throws, triggering the double-error path
    // where both op_http_metric_handle_otel_error and
    // op_http_copy_span_to_otel_info run on the same request.
    throw new Error("onError error");
  },
});
