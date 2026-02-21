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

const done = Promise.withResolvers<void>();

const ws = new WebSocket("http://_/crons", { client });
ws.onmessage = (e) => {
  console.log(e.data);
  const { id } = JSON.parse(e.data);
  if (id === 2) {
    fetch("http://_/crons", {
      method: "POST",
      client,
      body: JSON.stringify({ name: "Fail cron", id: 3 }),
    })
      .then((r) => r.text())
      .then(console.log);
  }
  if (id === 3) {
    done.resolve();
  }
};
await new Promise((r) => ws.onopen = r);

console.log(await fetch("http://_/crons", { client }).then((r) => r.json()));
console.log(
  await fetch("http://_/crons", {
    method: "POST",
    client,
    body: JSON.stringify({ name: "does not exist", id: 1 }),
  }).then((r) => r.text()),
);
console.log(
  await fetch("http://_/crons", {
    method: "POST",
    client,
    body: JSON.stringify({ name: "A fun cron 123 - _", id: 2 }),
  }).then((r) => r.text()),
);
await done.promise;
