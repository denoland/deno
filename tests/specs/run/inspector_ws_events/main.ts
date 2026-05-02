// Get an open port
const listener = Deno.listen({ port: 0 });
const port = (listener.addr as Deno.NetAddr).port;
listener.close();

new Deno.Command(Deno.execPath(), {
  args: ["run", "--inspect=127.0.0.1:" + port, "--watch", "a.ts"],
  stdin: "null",
  stdout: "null",
  stderr: "null",
}).spawn();

// Wait a bit for the inspector server to be ready
await new Promise((r) => setTimeout(r, 500));

const ws = new WebSocket(`ws://127.0.0.1:${port}/ws/events`);
ws.onopen = async () => {
  console.log("connected");
  await Deno.writeTextFile("a.ts", `console.log("bar");`);
};
ws.onmessage = (msg) => {
  console.log("message", msg.data);
  Deno.exit();
};

ws.onerror = (e) => {
  console.log("connection_failed");
  Deno.exit();
};
