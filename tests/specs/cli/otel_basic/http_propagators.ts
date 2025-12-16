import { assertEquals } from "@std/assert";
import { join } from "@std/path";

const tmpDir = Deno.makeTempDirSync();

const file1 = join(tmpDir, "1");
const command1 = new Deno.Command(Deno.execPath(), {
  env: {
    "OTEL_DENO_METRICS": "false",
    "OTEL_SERVICE_NAME": "server_1",
    "DENO_UNSTABLE_OTEL_DETERMINISTIC": "1",
  },
  args: ["run", "-A", "-q", "http_propagators_1.ts", file1],
  stdout: "inherit",
  stderr: "inherit",
});

const p1 = command1.output();

const file2 = join(tmpDir, "2");
const command2 = new Deno.Command(Deno.execPath(), {
  env: {
    "OTEL_DENO_METRICS": "false",
    "OTEL_SERVICE_NAME": "server_2",
    "DENO_UNSTABLE_OTEL_DETERMINISTIC": "2",
  },
  args: ["run", "-A", "-q", "http_propagators_2.ts", file2],
  stdout: "inherit",
  stderr: "inherit",
});

const p2 = command2.output();

// Wait until both file1 and file2 are created, but at most 10 seconds
const start = Date.now();
while (true) {
  const file1Exists = await Deno.stat(file1).then(() => true)
    .catch(() => false);
  const file2Exists = await Deno.stat(file2).then(() => true)
    .catch(() => false);
  console.log({ file1Exists, file2Exists });
  if (file1Exists && file2Exists) break;
  if (Date.now() - start > 10000) {
    throw new Error("Timeout waiting for servers to start");
  }
  await new Promise((resolve) => setTimeout(resolve, 100));
}

console.log("Both servers started");

const res = await fetch("http://localhost:8000", {
  headers: { "baggage": "userId=alice" },
});
assertEquals(await res.text(), "Hello World!");

await Promise.all([p1, p2]);
