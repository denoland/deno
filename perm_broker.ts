#!/usr/bin/env -S deno run --allow-net --allow-read --allow-write

interface PermissionAuditMessage {
  v: number;
  datetime: string;
  permission: string;
  value: string;
  stack?: string[];
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
    const buffer = new Uint8Array(4096);
    while (true) {
      const bytesRead = await conn.read(buffer);
      if (bytesRead === null) {
        break;
      }

      const message = new TextDecoder().decode(buffer.subarray(0, bytesRead));
      
      try {
        const auditMessage: PermissionAuditMessage = JSON.parse(message);
        console.log("Received permission audit message:", auditMessage);
        
        const response = JSON.stringify({});
        await conn.write(new TextEncoder().encode(response + "\n"));
      } catch (parseError) {
        console.error("Failed to parse message:", parseError);
        const errorResponse = JSON.stringify({});
        await conn.write(new TextEncoder().encode(errorResponse + "\n"));
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

function main(): void {
  const args = Deno.args;
  
  if (args.length !== 1) {
    console.error("Usage: deno run --allow-net --allow-read --allow-write perm_broker.ts <socket_path>");
    Deno.exit(1);
  }

  const socketPath = args[0];
  createUnixSocketServer(socketPath).catch((error) => {
    console.error("Server error:", error);
    Deno.exit(1);
  });
}

if (import.meta.main) {
  main();
}