#!/usr/bin/env -S deno run -REN

interface PermissionAuditRequest {
  id: number;
  v: number;
  datetime: string;
  permission: string;
  value: string;
}

async function createUnixSocketServer(socketPath: string): Promise<void> {
  try {
    await Deno.remove(socketPath);
  } catch {
    // Socket file doesn't exist, which is fine
  }

  const listener = Deno.listen({ transport: "unix", path: socketPath });
  console.log(`Permission broker listening on Unix socket: ${socketPath}`);

  for await (const conn of listener) {
    handleConnection(conn);
  }
}

async function handleConnection(conn: Deno.Conn): Promise<void> {
  console.log("New client connected");

  try {
    while (true) {
      const buffer = new Uint8Array(4096);

      const bytesRead = await conn.read(buffer);
      if (bytesRead === null) {
        console.log("Client closed connection");
        return;
      }

      const newlineIndex = buffer.indexOf(0x0A); // ASCII code for '\n'

      if (newlineIndex === -1) {
        throw new Error("Message is not a JSONL");
      }
      const messageBytes = buffer.subarray(0, newlineIndex);
      const message = new TextDecoder().decode(messageBytes);
      const request = JSON.parse(message) as PermissionAuditRequest;
      const response = { id: request.id, result: "allow" };
      if (request.permission === "env") {
        response.result = "deny";
      }
      const buf = new TextEncoder().encode(JSON.stringify(response));
      const written = await conn.write(buf);
      if (written != buf.length) {
        throw new Error("Bad write");
      }
    }
  } catch (error) {
    console.error("Connection error:", error);
  } finally {
    try {
      conn.close();
    } catch {
      // Connection might already be closed
    }
    console.log("Client disconnected");
  }
}

const args = Deno.args;

if (args.length !== 1) {
  console.error(
    "Usage: deno run -REN perm_broker.ts <socket_path>",
  );
  Deno.exit(1);
}

createUnixSocketServer(args[0]).catch((error) => {
  console.error("Server error:", error);
  Deno.exit(1);
});
