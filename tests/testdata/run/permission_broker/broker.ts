#!/usr/bin/env -S deno run -A

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
        console.log(message);
        const request = JSON.parse(message) as PermissionAuditRequest;
        const response = { id: request.id, result: "allow" };
        if (request.permission === "env") {
          response.result = "deny";
          response.reason = "Make sure to enable reading env vars.";
        }
        const buf = new TextEncoder().encode(JSON.stringify(response) + "\n");
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
}

async function createWindowsNamedPipeServer(spec: string): Promise<void> {
  const pipePath = normalizePipePath(spec);

  // Deno FFI bindings
  const k32 = Deno.dlopen(
    "kernel32.dll",
    {
      CreateNamedPipeW: {
        parameters: [
          "buffer",
          "u32",
          "u32",
          "u32",
          "u32",
          "u32",
          "u32",
          "pointer",
        ],
        result: "pointer",
      },
      ConnectNamedPipe: { parameters: ["pointer", "pointer"], result: "i32" },
      ReadFile: {
        parameters: ["pointer", "buffer", "u32", "buffer", "pointer"],
        result: "i32",
      },
      WriteFile: {
        parameters: ["pointer", "buffer", "u32", "buffer", "pointer"],
        result: "i32",
      },
      FlushFileBuffers: { parameters: ["pointer"], result: "i32" },
      DisconnectNamedPipe: { parameters: ["pointer"], result: "i32" },
      CloseHandle: { parameters: ["pointer"], result: "i32" },
      GetLastError: { parameters: [], result: "u32" },
    } as const,
  );

  // constants
  const PIPE_ACCESS_DUPLEX = 0x00000003; // read/write
  const PIPE_TYPE_BYTE = 0x00000000; // byte stream
  const PIPE_READMODE_BYTE = 0x00000000; // read mode = byte
  const PIPE_WAIT = 0x00000000; // blocking
  const PIPE_UNLIMITED_INSTANCES = 255;
  const ERROR_PIPE_CONNECTED = 535;
  const ERROR_BROKEN_PIPE = 109;

  const pathW = toWideCString(pipePath);

  // Sequential, one client at a time (simple & reliable without overlapped I/O).
  while (true) {
    // Create one pipe instance and wait for a client.
    const hPipe = k32.symbols.CreateNamedPipeW(
      pathW,
      PIPE_ACCESS_DUPLEX,
      PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
      PIPE_UNLIMITED_INSTANCES,
      65536, // out buf
      65536, // in buf
      0, // default timeout
      null,
    );
    console.log(`Permission broker listening on Windows pipe: ${pipePath}`);

    if (isInvalidHandle(hPipe)) {
      const code = k32.symbols.GetLastError();
      throw new Error(`CreateNamedPipeW failed: ${code}`);
    }

    // Wait for a client to connect
    const ok = k32.symbols.ConnectNamedPipe(hPipe, null);
    if (ok === 0) {
      const code = k32.symbols.GetLastError();
      if (code !== ERROR_PIPE_CONNECTED) {
        k32.symbols.CloseHandle(hPipe);
        throw new Error(`ConnectNamedPipe failed: ${code}`);
      }
      // else: already connected, proceed
    }

    try {
      // Handle this client
      handlePipeClient(hPipe);
    } catch (err) {
      console.error("Pipe client error:", err);
    } finally {
      // Flush, disconnect, close
      k32.symbols.FlushFileBuffers(hPipe);
      k32.symbols.DisconnectNamedPipe(hPipe);
      k32.symbols.CloseHandle(hPipe);
    }
  }

  function handlePipeClient(hPipe: Deno.PointerValue): void {
    console.log("New client connected");
    const decoder = new TextDecoder();
    const encoder = new TextEncoder();

    const readBuf = new Uint8Array(4096);
    const nRead = new Uint32Array(1);
    const nReadBuf = new Uint8Array(nRead.buffer);

    let pending = new Uint8Array(0);

    // Synchronous loop; each ReadFile blocks this JS thread until data arrives.
    for (;;) {
      const ok = k32.symbols.ReadFile(
        hPipe,
        readBuf,
        readBuf.byteLength,
        nReadBuf,
        null,
      );
      if (ok === 0) {
        const code = k32.symbols.GetLastError();
        if (code === ERROR_BROKEN_PIPE) {
          console.log("Client closed connection");
          break;
        }
        throw new Error(`ReadFile failed: ${code}`);
      }

      const n = nRead[0] >>> 0;
      if (n === 0) continue;

      pending = concat(pending, readBuf.subarray(0, n));

      while (true) {
        const i = pending.indexOf(0x0A); // '\n'
        if (i === -1) break;

        const line = pending.subarray(0, i);
        pending = pending.subarray(i + 1);

        const message = decoder.decode(line);
        console.log(message);
        const request = JSON.parse(message) as PermissionAuditRequest;

        const response = {
          id: request.id,
          result: request.permission === "env" ? "deny" : "allow",
          reason: request.permission === "env"
            ? "Make sure to enable reading env vars."
            : undefined,
        };
        const out = encoder.encode(JSON.stringify(response) + "\n");
        writeAll(hPipe, out);
      }
    }
    console.log("Client disconnected");
  }

  function writeAll(hPipe: Deno.PointerValue, data: Uint8Array): void {
    const nWrote = new Uint32Array(1);
    const nWroteBuf = new Uint8Array(nWrote.buffer);

    let off = 0;
    while (off < data.length) {
      const chunk = data.subarray(off);
      const ok = k32.symbols.WriteFile(
        hPipe,
        chunk,
        chunk.byteLength,
        nWroteBuf,
        null,
      );
      if (ok === 0) {
        const code = k32.symbols.GetLastError();
        throw new Error(`WriteFile failed: ${code}`);
      }
      off += nWrote[0] >>> 0;
      if (nWrote[0] === 0) throw new Error("WriteFile wrote 0 bytes");
    }
  }

  function toWideCString(s: string): Uint16Array {
    const u = new Uint16Array(s.length + 1);
    for (let i = 0; i < s.length; i++) u[i] = s.charCodeAt(i);
    u[s.length] = 0; // NUL
    return u;
  }

  function normalizePipePath(spec: string): string {
    // Accept either a bare name ("deno-permission-broker") or a full path.
    if (spec.startsWith("\\\\.\\")) return spec;
    return `\\\\.\\pipe\\${spec}`;
  }

  function isInvalidHandle(h: Deno.PointerValue): boolean {
    // INVALID_HANDLE_VALUE == (void*)-1
    if (typeof h === "bigint") {
      return h === 0n || h === 0xffffffffffffffffn;
    }
    // 32-bit fallback
    return h === 0 || h === 0xffffffff;
  }

  function concat(
    a: Uint8Array<ArrayBuffer>,
    b: Uint8Array<ArrayBuffer>,
  ): Uint8Array<ArrayBuffer> {
    const out = new Uint8Array(a.length + b.length);
    out.set(a, 0);
    out.set(b, a.length);
    return out;
  }
}

const args = Deno.args;

if (args.length !== 1) {
  console.error("Usage: deno run -A broker.ts <socket_path>");
  Deno.exit(1);
}

const socketPath = args[0];
const promise = Deno.build.os === "windows"
  ? createWindowsNamedPipeServer(socketPath)
  : createUnixSocketServer(socketPath);
promise.catch((error) => {
  console.error("Server error:", error);
  Deno.exit(1);
});
