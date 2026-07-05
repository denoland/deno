// Regression test: node:inspector waitForDebugger() must block until a
// session sends Runtime.runIfWaitingForDebugger and then resume WITHOUT
// scheduling a pause on the next statement. It used the --inspect-brk
// primitive, so clients received an unexpected Debugger.paused right after
// attaching (VS Code stopped inside js-debug's bootloader).

const preload = `${Deno.cwd()}/preload.cjs`;
const program = `${Deno.cwd()}/program.js`;
Deno.writeTextFileSync(
  preload,
  'const inspector = require("node:inspector");\n' +
    "inspector.open(0);\n" +
    "inspector.waitForDebugger();\n",
);
Deno.writeTextFileSync(program, 'console.log("program ran");\n');

const deadline = setTimeout(() => {
  console.error("test timed out waiting for the program to resume");
  Deno.exit(1);
}, 60_000);
Deno.unrefTimer(deadline);

const child = new Deno.Command(Deno.execPath(), {
  args: ["run", "-A", "--require", preload, program],
  stdout: "piped",
  stderr: "piped",
  env: { NO_COLOR: "1" },
}).spawn();

// Track whether the program's stdout marker arrived; waitForDebugger() must
// hold it back until we send Runtime.runIfWaitingForDebugger.
let programRan = false;
let notifyRan = () => {};
(async () => {
  let buffered = "";
  for await (const chunk of child.stdout.pipeThrough(new TextDecoderStream())) {
    buffered += chunk;
    let newlineIndex;
    while ((newlineIndex = buffered.indexOf("\n")) >= 0) {
      const line = buffered.slice(0, newlineIndex);
      buffered = buffered.slice(newlineIndex + 1);
      if (line === "program ran") {
        programRan = true;
        notifyRan();
      }
    }
  }
})();

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

let paused = false;
ws.onmessage = (e) => {
  const message = JSON.parse(e.data);
  if (message.method === "Debugger.paused") {
    paused = true;
    ws.send(JSON.stringify({ id: 99, method: "Debugger.resume" }));
  }
};
ws.send(JSON.stringify({ id: 1, method: "Runtime.enable" }));
ws.send(JSON.stringify({ id: 2, method: "Debugger.enable" }));

// Give the child a moment: the program must NOT run before the session sends
// Runtime.runIfWaitingForDebugger.
await new Promise((resolve) => setTimeout(resolve, 1000));
if (programRan) {
  console.error("waitForDebugger() did not block until the debugger resumed");
  Deno.exit(1);
}
console.log("program blocked until runIfWaitingForDebugger");

ws.send(JSON.stringify({ id: 3, method: "Runtime.runIfWaitingForDebugger" }));

await new Promise((resolve) => {
  if (programRan) resolve();
  else notifyRan = resolve;
});

// Allow any (incorrectly) scheduled pause to surface before checking.
await new Promise((resolve) => setTimeout(resolve, 500));
if (paused) {
  console.error("received an unexpected Debugger.paused after resuming");
  Deno.exit(1);
}
console.log("program resumed without pausing");

ws.close();
await child.status;
