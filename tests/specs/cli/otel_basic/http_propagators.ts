import { assertEquals } from "@std/assert";

const command1 = new Deno.Command(Deno.execPath(), {
  env: {
    "OTEL_DENO_METRICS": "false",
    "OTEL_SERVICE_NAME": "server_1",
    "DENO_UNSTABLE_OTEL_DETERMINISTIC": "1",
  },
  args: ["run", "-A", "--unstable-otel", "http_propagators_1.ts"],
});

const p1 = command1.output();

await new Promise((resolve) => setTimeout(resolve, 1000));

const command2 = new Deno.Command(Deno.execPath(), {
  env: {
    "OTEL_DENO_METRICS": "false",
    "OTEL_SERVICE_NAME": "server_2",
    "DENO_UNSTABLE_OTEL_DETERMINISTIC": "2",
  },
  args: ["run", "-A", "--unstable-otel", "http_propagators_2.ts"],
});

const p2 = command2.output();

await new Promise((resolve) => setTimeout(resolve, 1000));

const res = await fetch("http://localhost:8000");
assertEquals(await res.text(), "Hello World!");

await Promise.all([p1, p2]);
