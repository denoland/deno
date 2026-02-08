import { join } from "@std/path";

const socketPath = join(Deno.makeTempDirSync(), "test.sock");

await using _child = new Deno.Command(Deno.execPath(), {
  env: {
    DENO_UNSTABLE_CRON_SOCK: `unix:${socketPath}`,
  },
  args: ["run", "-A", "--unstable-cron", "child.ts"],
  stdin: "null",
  stdout: "inherit",
  stderr: "inherit",
}).spawn();

while (true) {
  try {
    Deno.statSync(socketPath);
    break;
  } catch {}
}

const client = Deno.createHttpClient({
  proxy: {
    transport: "unix",
    path: socketPath,
  },
});

const ws = new WebSocket("ws://cron", { client });

const done = Promise.withResolvers<void>();

ws.addEventListener("message", (event) => {
  console.log(event.data);
  const { type, ...args } = JSON.parse(event.data);
  if (args.id === 1) {
    ws.send(JSON.stringify({ type: "execute", id: 2, name: "fail-cron" }));
  }
  if (args.id === 2) {
    done.resolve();
  }
});

ws.addEventListener("open", () => {
  ws.send(JSON.stringify({ type: "execute", id: 1, name: "success-cron" }));
});

await done.promise;
