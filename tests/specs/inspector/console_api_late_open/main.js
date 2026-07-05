// Regression test: when the inspector is activated at runtime via
// node:inspector open() (instead of a CLI --inspect* flag), sessions must
// still receive Runtime.consoleAPICalled for console calls. The console was
// only bridged to the V8 inspector console when an --inspect* flag was
// present at bootstrap, so late-opened inspectors saw no console output.

const preload = `${Deno.cwd()}/preload.cjs`;
const logger = `${Deno.cwd()}/logger.js`;
Deno.writeTextFileSync(preload, 'require("node:inspector").open(0);\n');
Deno.writeTextFileSync(
  logger,
  'setInterval(() => console.log("tick"), 100);\n',
);

const deadline = setTimeout(() => {
  console.error("test timed out waiting for Runtime.consoleAPICalled");
  Deno.exit(1);
}, 60_000);
Deno.unrefTimer(deadline);

const child = new Deno.Command(Deno.execPath(), {
  args: ["run", "-A", "--require", preload, logger],
  stdout: "null",
  stderr: "piped",
  env: { NO_COLOR: "1" },
}).spawn();

// Scan the child's stderr for the "Debugger listening on ws://..." line and
// keep draining the stream so the child can't block on a full pipe.
const wsUrl = await new Promise((resolve) => {
  (async () => {
    let buffered = "";
    for await (
      const chunk of child.stderr.pipeThrough(new TextDecoderStream())
    ) {
      buffered += chunk;
      let newlineIndex;
      while ((newlineIndex = buffered.indexOf("\n")) >= 0) {
        const line = buffered.slice(0, newlineIndex);
        buffered = buffered.slice(newlineIndex + 1);
        const match = line.match(/Debugger listening on (ws:\/\/\S+)/);
        if (match) resolve(match[1]);
      }
    }
  })();
});

const ws = new WebSocket(wsUrl);
await new Promise((resolve, reject) => {
  ws.onopen = resolve;
  ws.onerror = () => reject(new Error("failed to connect to inspector"));
});
console.log("connected");

ws.send(JSON.stringify({ id: 1, method: "Runtime.enable" }));

await new Promise((resolve) => {
  ws.onmessage = (e) => {
    const message = JSON.parse(e.data);
    if (
      message.method === "Runtime.consoleAPICalled" &&
      message.params.args[0]?.value === "tick"
    ) {
      resolve();
    }
  };
});
console.log("received Runtime.consoleAPICalled for console.log");

ws.close();
child.kill("SIGTERM");
await child.status;
