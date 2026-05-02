import { trace } from "npm:@opentelemetry/api@1.9.0";

let port: number;
const server = Deno.serve(
  {
    port: 0,
    onListen: (addr) => {
      port = addr.port;
    },
  },
  (req) => {
    const url = new URL(req.url);
    // Simulate what a router framework would do: set http.route on the active span
    const span = trace.getActiveSpan();
    if (span && url.pathname.startsWith("/users/")) {
      span.setAttribute("http.route", "/users/:id");
    }
    return new Response("ok");
  },
);

for (let i = 0; i < 2; i++) {
  await (await fetch(`http://localhost:${port!}/users/${i}`)).text();
}

await server.shutdown();
