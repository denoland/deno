// Mock external cron service
const socketPath = Deno.env.get("TEST_SOCKET_PATH");

const listener = Deno.listen({ transport: "unix", path: socketPath });

const conn = await listener.accept();
const reader = conn.readable.pipeThrough(new TextDecoderStream()).getReader();
const writer = conn.writable.getWriter();

let firstCronName = null;
let hasInvoked = false;
let buffer = "";

outer: while (true) {
  const { value: chunk, done } = await reader.read();
  if (done) {
    console.error("[CRON SERVER] Connection closed");
    break;
  }

  buffer += chunk;
  const lines = buffer.split("\n");
  buffer = lines.pop();

  for (const line of lines) {
    if (!line.trim()) continue;

    console.error("[CRON SERVER] Received:", line);

    const msg = JSON.parse(line);

    if (msg.kind === "register") {
      console.error("[CRON SERVER] Registered", msg.crons.length, "crons");
      console.error(
        "[CRON SERVER] Cron names:",
        msg.crons.map((c) => c.name).join(", "),
      );

      if (!firstCronName) {
        firstCronName = msg.crons[0].name;
      }

      if (!hasInvoked && firstCronName) {
        hasInvoked = true;
        setTimeout(async () => {
          const invocation =
            JSON.stringify({ kind: "invoke", name: firstCronName }) + "\n";
          console.error("[CRON SERVER] Invoking cron:", firstCronName);
          await writer.write(new TextEncoder().encode(invocation));
        }, 200);
      }
    } else if (msg.kind === "result") {
      console.error(
        "[CRON SERVER] Result for",
        msg.name + ":",
        msg.success ? "SUCCESS" : "FAILURE",
      );
      break outer;
    }
  }
}

try {
  conn.close();
} catch {
  // Connection may already be closed
}
listener.close();

console.error("[CRON SERVER] Done");
