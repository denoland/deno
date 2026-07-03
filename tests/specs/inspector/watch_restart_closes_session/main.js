// Regression test: when the file watcher restarts the runtime, the global
// inspector server must close the WebSocket connections of the torn-down
// session. Otherwise debugger clients are left attached to a socket that
// stays open but silently ignores all messages, so they can never detect
// the restart and reconnect (broken since 2.7.0).

const watched = `${Deno.cwd()}/watched.js`;
Deno.writeTextFileSync(watched, "setInterval(() => {}, 1000);\n");

const deadline = setTimeout(() => {
  console.error("test timed out waiting for the inspector to close the socket");
  Deno.exit(1);
}, 60_000);
Deno.unrefTimer(deadline);

const child = new Deno.Command(Deno.execPath(), {
  args: ["run", "--watch", "--inspect=127.0.0.1:0", watched],
  stdout: "null",
  stderr: "piped",
  env: { NO_COLOR: "1" },
}).spawn();

// Scan the child's stderr for "Debugger listening on ws://..." lines and keep
// draining the stream afterwards so the child can't block on a full pipe.
const wsUrls = [];
let notifyUrl = () => {};
(async () => {
  let buffered = "";
  for await (const chunk of child.stderr.pipeThrough(new TextDecoderStream())) {
    buffered += chunk;
    let newlineIndex;
    while ((newlineIndex = buffered.indexOf("\n")) >= 0) {
      const line = buffered.slice(0, newlineIndex);
      buffered = buffered.slice(newlineIndex + 1);
      const match = line.match(/Debugger listening on (ws:\/\/\S+)/);
      if (match) {
        wsUrls.push(match[1]);
        notifyUrl();
      }
    }
  }
})();

async function waitForWsUrlCount(count) {
  while (wsUrls.length < count) {
    await new Promise((resolve) => notifyUrl = resolve);
  }
}

await waitForWsUrlCount(1);
const ws = new WebSocket(wsUrls[0]);
const closed = new Promise((resolve) => ws.onclose = (e) => resolve(e));
await new Promise((resolve, reject) => {
  ws.onopen = resolve;
  ws.onerror = () => reject(new Error("failed to connect to inspector"));
});
console.log("connected");

// Trigger watcher restarts until the server closes our socket; re-writing
// the file guards against a change event being missed or debounced away.
let socketClosed = false;
closed.then(() => socketClosed = true);
(async () => {
  for (let i = 0; !socketClosed; i++) {
    Deno.writeTextFileSync(watched, `setInterval(() => {}, 1000); // ${i}\n`);
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
})();

const closeEvent = await closed;
console.log(
  `websocket closed on watch restart: ${closeEvent.code} ${closeEvent.reason}`,
);

// The restarted runtime must expose a connectable session again. The touch
// loop above may have queued more than one restart, so always try the most
// recently announced URL and retry while the watcher settles.
await waitForWsUrlCount(2);
let reconnected = false;
for (let attempt = 0; attempt < 10 && !reconnected; attempt++) {
  const ws2 = new WebSocket(wsUrls[wsUrls.length - 1]);
  reconnected = await new Promise((resolve) => {
    ws2.onopen = () => {
      ws2.close();
      resolve(true);
    };
    ws2.onerror = () => resolve(false);
  });
  if (!reconnected) {
    await new Promise((resolve) => setTimeout(resolve, 300));
  }
}
if (!reconnected) {
  console.error("failed to reconnect to the restarted runtime");
  Deno.exit(1);
}
console.log("reconnected to new session");

child.kill();
await child.status;
Deno.exit(0);
