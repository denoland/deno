// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals, assertStringIncludes } from "./test_util.ts";

const testdataPath =
  new URL("../testdata/inspector/", import.meta.url).pathname;

interface CDPMessage {
  id?: number;
  method?: string;
  result?: unknown;
  params?: unknown;
  error?: { code: number; message: string };
}

interface InspectorTesterOptions {
  notificationFilter?: (msg: CDPMessage) => boolean;
  timeout?: number;
  env?: Record<string, string>;
  cwd?: string;
}

const DEFAULT_CDP_TIMEOUT = 60_000;

let nextPort = 9229;
function inspectFlagWithUniquePort(flagPrefix: string): string {
  const port = nextPort++;
  return `${flagPrefix}=127.0.0.1:${port}`;
}

function ignoreScriptParsed(msg: CDPMessage): boolean {
  return msg.method !== "Debugger.scriptParsed";
}

class InspectorTester {
  private socket: WebSocket;
  private child: Deno.ChildProcess;
  private stderrReader: ReadableStreamDefaultReader<string>;
  private stdoutReader: ReadableStreamDefaultReader<string>;
  private stderrBuffer: string = "";
  private stdoutBuffer: string = "";
  private responseBuffer: Map<number, CDPMessage> = new Map();
  private notificationBuffer: CDPMessage[] = [];
  private messageWaiters: Array<() => void> = [];
  private socketClosed = false;
  private receiverTask: Promise<void>;
  private notificationFilter: (msg: CDPMessage) => boolean;
  private timeout: number;

  private constructor(
    socket: WebSocket,
    child: Deno.ChildProcess,
    stderrReader: ReadableStreamDefaultReader<string>,
    stdoutReader: ReadableStreamDefaultReader<string>,
    notificationFilter: (msg: CDPMessage) => boolean,
    timeout: number,
    responseBuffer: Map<number, CDPMessage>,
    notificationBuffer: CDPMessage[],
    messageWaiters: Array<() => void>,
  ) {
    this.socket = socket;
    this.child = child;
    this.stderrReader = stderrReader;
    this.stdoutReader = stdoutReader;
    this.notificationFilter = notificationFilter;
    this.timeout = timeout;
    this.responseBuffer = responseBuffer;
    this.notificationBuffer = notificationBuffer;
    this.messageWaiters = messageWaiters;
    this.receiverTask = this.setupSocketCloseHandler();
  }

  static async create(
    args: string[],
    options?: InspectorTesterOptions,
  ): Promise<InspectorTester> {
    const notificationFilter = options?.notificationFilter ?? (() => true);
    const timeout = options?.timeout ?? DEFAULT_CDP_TIMEOUT;

    const command = new Deno.Command(Deno.execPath(), {
      args,
      stdin: "piped",
      stdout: "piped",
      stderr: "piped",
      env: options?.env,
      cwd: options?.cwd,
    });

    const child = command.spawn();

    const stderrReader = child.stderr.pipeThrough(new TextDecoderStream())
      .getReader();

    const stdoutReader = child.stdout.pipeThrough(new TextDecoderStream())
      .getReader();

    const { wsUrl, remainingBuffer } = await InspectorTester.extractWsUrl(
      stderrReader,
    );

    // Small delay to work around timing
    // if we connect too quickly after a previous
    await new Promise((r) => setTimeout(r, 100));

    const socket = new WebSocket(wsUrl);
    const responseBuffer = new Map<number, CDPMessage>();
    const notificationBuffer: CDPMessage[] = [];
    const messageWaiters: Array<() => void> = [];

    // Set up message handler before waiting for open to avoid race conditions
    socket.onmessage = (event) => {
      const msg: CDPMessage = JSON.parse(event.data as string);
      if (msg.method && !notificationFilter(msg)) {
        return;
      }

      if (msg.id !== undefined) {
        responseBuffer.set(msg.id, msg);
      } else {
        notificationBuffer.push(msg);
      }

      for (const waiter of messageWaiters) {
        waiter();
      }
      messageWaiters.length = 0;
    };

    await new Promise<void>((resolve, reject) => {
      socket.onopen = () => resolve();
      socket.onerror = (e) => reject(e);
    });

    const tester = new InspectorTester(
      socket,
      child,
      stderrReader,
      stdoutReader,
      notificationFilter,
      timeout,
      responseBuffer,
      notificationBuffer,
      messageWaiters,
    );
    tester.stderrBuffer = remainingBuffer;
    return tester;
  }

  private static async extractWsUrl(
    reader: ReadableStreamDefaultReader<string>,
  ): Promise<{ wsUrl: string; remainingBuffer: string }> {
    let buffer = "";
    const deadline = Date.now() + 30_000;

    while (Date.now() < deadline) {
      const { value, done } = await reader.read();
      if (done) {
        throw new Error("Stderr closed before WebSocket URL found");
      }
      buffer += value;

      const lines = buffer.split("\n");
      for (let i = 0; i < lines.length; i++) {
        const line = lines[i];
        if (line.startsWith("Debugger listening on ")) {
          const wsUrl = line.slice("Debugger listening on ".length).trim();
          const remainingBuffer = lines.slice(i + 1).join("\n");
          return { wsUrl, remainingBuffer };
        }
      }
    }

    throw new Error("Timeout waiting for WebSocket URL in stderr");
  }

  private setupSocketCloseHandler(): Promise<void> {
    return new Promise<void>((resolve) => {
      this.socket.onclose = () => {
        this.socketClosed = true;
        for (const waiter of this.messageWaiters) {
          waiter();
        }
        resolve();
      };

      this.socket.onerror = () => {
        this.socketClosed = true;
        for (const waiter of this.messageWaiters) {
          waiter();
        }
        resolve();
      };
    });
  }

  private waitForMessage(timeoutMs: number): Promise<void> {
    return new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        const idx = this.messageWaiters.indexOf(waiter);
        if (idx !== -1) {
          this.messageWaiters.splice(idx, 1);
        }
        reject(new Error("Timeout waiting for message"));
      }, timeoutMs);

      const waiter = () => {
        clearTimeout(timer);
        resolve();
      };
      this.messageWaiters.push(waiter);
    });
  }

  send(message: Record<string, unknown>): void {
    this.socket.send(JSON.stringify(message));
  }

  sendMany(messages: Record<string, unknown>[]): void {
    for (const msg of messages) {
      this.send(msg);
    }
  }

  async expectResponse(
    id: number,
    options?: { prefixMatch?: string; timeout?: number },
  ): Promise<CDPMessage> {
    const timeoutMs = options?.timeout ?? this.timeout;
    const deadline = Date.now() + timeoutMs;

    while (Date.now() < deadline) {
      if (this.responseBuffer.has(id)) {
        const msg = this.responseBuffer.get(id)!;
        this.responseBuffer.delete(id);

        if (options?.prefixMatch) {
          const json = JSON.stringify(msg);
          if (!json.startsWith(options.prefixMatch)) {
            throw new Error(
              `Response ${id} doesn't match prefix. Expected: ${options.prefixMatch}, Got: ${json}`,
            );
          }
        }
        return msg;
      }

      if (this.socketClosed) {
        throw new Error(`Socket closed while waiting for response id=${id}`);
      }

      try {
        await this.waitForMessage(Math.min(1000, deadline - Date.now()));
      } catch {
        // continue
      }
    }

    throw new Error(`Timeout waiting for response id=${id}`);
  }

  async expectNotification(
    method: string,
    options?: { prefixMatch?: string; timeout?: number },
  ): Promise<CDPMessage> {
    const timeoutMs = options?.timeout ?? this.timeout;
    const deadline = Date.now() + timeoutMs;

    while (Date.now() < deadline) {
      const idx = this.notificationBuffer.findIndex((n) => n.method === method);
      if (idx !== -1) {
        const [msg] = this.notificationBuffer.splice(idx, 1);
        if (options?.prefixMatch) {
          const json = JSON.stringify(msg);
          if (!json.startsWith(options.prefixMatch)) {
            throw new Error(
              `Notification ${method} doesn't match prefix. Expected: ${options.prefixMatch}, Got: ${json}`,
            );
          }
        }
        return msg;
      }

      if (this.socketClosed) {
        throw new Error(
          `Socket closed while waiting for notification method=${method}`,
        );
      }

      try {
        await this.waitForMessage(Math.min(1000, deadline - Date.now()));
      } catch {
        // continue
      }
    }

    throw new Error(`Timeout waiting for notification method=${method}`);
  }

  async nextStdoutLine(): Promise<string> {
    const deadline = Date.now() + this.timeout;

    while (Date.now() < deadline) {
      const newlineIdx = this.stdoutBuffer.indexOf("\n");
      if (newlineIdx !== -1) {
        const line = this.stdoutBuffer.slice(0, newlineIdx);
        this.stdoutBuffer = this.stdoutBuffer.slice(newlineIdx + 1);
        return line;
      }

      const { value, done } = await this.stdoutReader.read();
      if (done) {
        if (this.stdoutBuffer.length > 0) {
          const line = this.stdoutBuffer;
          this.stdoutBuffer = "";
          return line;
        }
        throw new Error("Stdout closed while waiting for line");
      }
      this.stdoutBuffer += value;
    }

    throw new Error("Timeout waiting for stdout line");
  }

  async nextStderrLine(): Promise<string> {
    const deadline = Date.now() + this.timeout;

    while (Date.now() < deadline) {
      const newlineIdx = this.stderrBuffer.indexOf("\n");
      if (newlineIdx !== -1) {
        const line = this.stderrBuffer.slice(0, newlineIdx);
        this.stderrBuffer = this.stderrBuffer.slice(newlineIdx + 1);
        // deno-lint-ignore no-control-regex
        return line.replace(/\x1b\[[0-9;]*m/g, "");
      }

      const { value, done } = await this.stderrReader.read();
      if (done) {
        if (this.stderrBuffer.length > 0) {
          const line = this.stderrBuffer;
          this.stderrBuffer = "";
          // deno-lint-ignore no-control-regex
          return line.replace(/\x1b\[[0-9;]*m/g, "");
        }
        throw new Error("Stderr closed while waiting for line");
      }
      this.stderrBuffer += value;
    }

    throw new Error("Timeout waiting for stderr line");
  }

  async assertStderrForInspect(): Promise<void> {
    const line = await this.nextStderrLine();
    assertEquals(line, "Visit chrome://inspect to connect to the debugger.");
  }

  async assertStderrForInspectBrk(): Promise<void> {
    const line1 = await this.nextStderrLine();
    assertEquals(line1, "Visit chrome://inspect to connect to the debugger.");
    const line2 = await this.nextStderrLine();
    assertEquals(line2, "Deno is waiting for debugger to connect.");
  }

  async close(): Promise<void> {
    this.socket.close();
    await this.receiverTask;
  }

  kill(): void {
    try {
      this.child.kill();
    } catch {
      // already killed
    }
  }

  async waitForExit(): Promise<Deno.CommandStatus> {
    return await this.child.status;
  }

  get stdin(): WritableStream<Uint8Array> {
    return this.child.stdin;
  }
}

Deno.test({
  name: "inspector_connect",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const script = `${testdataPath}/inspector1.js`;
    const inspectFlag = inspectFlagWithUniquePort("--inspect");

    const command = new Deno.Command(Deno.execPath(), {
      args: ["run", inspectFlag, script],
      stderr: "piped",
    });

    const child = command.spawn();
    const stderrReader = child.stderr.pipeThrough(new TextDecoderStream())
      .getReader();

    let buffer = "";
    let wsUrl = "";
    while (true) {
      const { value, done } = await stderrReader.read();
      if (done) {
        break;
      }
      buffer += value;
      if (buffer.includes("Debugger listening on ")) {
        const match = buffer.match(/Debugger listening on (ws:\/\/[^\s]+)/);
        if (match) {
          wsUrl = match[1];
          break;
        }
      }
    }

    assert(wsUrl.startsWith("ws://"), "Should get WebSocket URL");

    const socket = new WebSocket(wsUrl);
    const connected = await new Promise<boolean>((resolve) => {
      socket.onopen = () => resolve(true);
      socket.onerror = () => resolve(false);
    });

    assert(connected, "WebSocket should connect successfully");

    socket.close();
    child.kill();
    await child.status;
  },
});

Deno.test({
  name: "inspector_break_on_first_line",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const script = `${testdataPath}/inspector2.js`;
    const inspectFlag = inspectFlagWithUniquePort("--inspect-brk");

    const command = new Deno.Command(Deno.execPath(), {
      args: ["run", inspectFlag, script],
      stdin: "piped",
      stdout: "piped",
      stderr: "piped",
    });

    const child = command.spawn();

    try {
      const stderrReader = child.stderr.pipeThrough(new TextDecoderStream())
        .getReader();
      const stdoutReader = child.stdout.pipeThrough(new TextDecoderStream())
        .getReader();

      // Extract WebSocket URL from stderr
      let stderrBuffer = "";
      let wsUrl = "";
      const urlDeadline = Date.now() + 30_000;
      while (Date.now() < urlDeadline) {
        const { value, done } = await stderrReader.read();
        if (done) {
          break;
        }
        stderrBuffer += value;
        const match = stderrBuffer.match(
          /Debugger listening on (ws:\/\/[^\s]+)/,
        );
        if (match) {
          wsUrl = match[1];
          break;
        }
      }
      assert(wsUrl, "Should get WebSocket URL");

      // Small delay to work around timing issues
      await new Promise((r) => setTimeout(r, 100));

      // Connect to WebSocket - set up message handler BEFORE waiting for open
      // to avoid missing any messages that arrive immediately after connection
      const socket = new WebSocket(wsUrl);
      const responses = new Map<number, string>();
      const notifications: string[] = [];
      socket.onmessage = (e) => {
        const msg = e.data as string;
        // Filter out scriptParsed
        if (msg.includes('"method":"Debugger.scriptParsed"')) {
          return;
        }
        const parsed = JSON.parse(msg);
        if (parsed.id !== undefined) {
          responses.set(parsed.id, msg);
        } else {
          notifications.push(msg);
        }
      };
      await new Promise<void>((resolve, reject) => {
        socket.onopen = () => resolve();
        socket.onerror = (e) => reject(e);
      });

      const waitForResponse = async (id: number, timeout = 10000) => {
        const deadline = Date.now() + timeout;
        while (Date.now() < deadline) {
          if (responses.has(id)) {
            return responses.get(id)!;
          }
          await new Promise((r) => setTimeout(r, 50));
        }
        throw new Error(`Timeout waiting for response ${id}`);
      };

      const waitForNotification = async (method: string, timeout = 10000) => {
        const deadline = Date.now() + timeout;
        while (Date.now() < deadline) {
          const idx = notifications.findIndex(
            (n) => n.includes(`"method":"${method}"`),
          );
          if (idx !== -1) {
            return notifications.splice(idx, 1)[0];
          }
          await new Promise((r) => setTimeout(r, 50));
        }
        throw new Error(`Timeout waiting for notification ${method}`);
      };

      // Enable runtime and debugger
      socket.send(JSON.stringify({ id: 1, method: "Runtime.enable" }));
      socket.send(JSON.stringify({ id: 2, method: "Debugger.enable" }));

      await waitForResponse(1);
      await waitForResponse(2);
      await waitForNotification("Runtime.executionContextCreated");

      // Run and expect pause
      socket.send(
        JSON.stringify({ id: 3, method: "Runtime.runIfWaitingForDebugger" }),
      );
      await waitForResponse(3);
      await waitForNotification("Debugger.paused");

      // Evaluate expression while paused
      socket.send(JSON.stringify({
        id: 4,
        method: "Runtime.evaluate",
        params: {
          expression:
            'Deno[Deno.internal].core.print("hello from the inspector\\n")',
          contextId: 1,
          includeCommandLineAPI: true,
          silent: false,
          returnByValue: true,
        },
      }));
      await waitForResponse(4);

      // Check stdout - read with timeout
      let stdoutBuffer = "";
      const stdoutDeadline = Date.now() + 10000;
      while (
        Date.now() < stdoutDeadline &&
        !stdoutBuffer.includes("hello from the inspector")
      ) {
        const result = await Promise.race([
          stdoutReader.read(),
          new Promise<{ value: undefined; done: true }>(
            (r) => setTimeout(() => r({ value: undefined, done: true }), 500),
          ),
        ]);
        if (result.value) {
          stdoutBuffer += result.value;
        }
        if (result.done && !result.value) {
          break;
        }
      }
      assert(
        stdoutBuffer.includes("hello from the inspector"),
        `Expected 'hello from the inspector' in stdout, got: ${stdoutBuffer}`,
      );

      // Resume
      socket.send(JSON.stringify({ id: 5, method: "Debugger.resume" }));
      await waitForResponse(5);

      // Script continues - read remaining stdout
      const finalDeadline = Date.now() + 10000;
      while (
        Date.now() < finalDeadline &&
        !stdoutBuffer.includes("hello from the script")
      ) {
        const result = await Promise.race([
          stdoutReader.read(),
          new Promise<{ value: undefined; done: true }>(
            (r) => setTimeout(() => r({ value: undefined, done: true }), 500),
          ),
        ]);
        if (result.value) {
          stdoutBuffer += result.value;
        }
        if (result.done && !result.value) {
          break;
        }
      }
      assert(
        stdoutBuffer.includes("hello from the script"),
        `Expected 'hello from the script' in stdout, got: ${stdoutBuffer}`,
      );

      socket.close();
    } finally {
      child.kill();
      await child.status;
    }
  },
});

Deno.test({
  name: "inspector_pause",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const script = `${testdataPath}/inspector1.js`;
    const tester = await InspectorTester.create(
      ["run", inspectFlagWithUniquePort("--inspect"), script],
      { notificationFilter: ignoreScriptParsed },
    );

    try {
      tester.send({ id: 6, method: "Debugger.enable" });
      await tester.expectResponse(6, {
        prefixMatch: '{"id":6,"result":{"debuggerId":',
      });

      tester.send({ id: 31, method: "Debugger.pause" });
      await tester.expectResponse(31, { prefixMatch: '{"id":31,"result":{}}' });
    } finally {
      tester.kill();
      await tester.waitForExit();
    }
  },
});

Deno.test({
  name: "inspector_port_collision",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    // Skip on Windows due to flakiness
    if (Deno.build.os === "windows") {
      return;
    }

    const script = `${testdataPath}/inspector1.js`;
    const inspectFlag = inspectFlagWithUniquePort("--inspect");

    // First process
    const command1 = new Deno.Command(Deno.execPath(), {
      args: ["run", inspectFlag, script],
      stderr: "piped",
    });
    const child1 = command1.spawn();
    const stderr1 = child1.stderr.pipeThrough(new TextDecoderStream())
      .getReader();

    // Wait for first process to start listening
    let buffer1 = "";
    while (true) {
      const { value, done } = await stderr1.read();
      if (done) {
        break;
      }
      buffer1 += value;
      if (buffer1.includes("Debugger listening on ")) {
        break;
      }
    }

    // Second process with same port - should fail
    const command2 = new Deno.Command(Deno.execPath(), {
      args: ["run", inspectFlag, script],
      stderr: "piped",
    });
    const child2 = command2.spawn();
    const stderr2 = child2.stderr.pipeThrough(new TextDecoderStream())
      .getReader();

    let buffer2 = "";
    while (true) {
      const { value, done } = await stderr2.read();
      if (done) {
        break;
      }
      buffer2 += value;
      if (
        buffer2.includes("Failed to start inspector server") ||
        buffer2.includes("error")
      ) {
        break;
      }
    }

    // Second process should fail with port collision error
    assert(
      !buffer2.includes("Debugger listening"),
      "Second process should not listen successfully",
    );

    child1.kill();
    await child1.status;
    await child2.status;
  },
});

Deno.test({
  name: "inspector_does_not_hang",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const script = `${testdataPath}/inspector3.js`;
    const tester = await InspectorTester.create(
      ["run", inspectFlagWithUniquePort("--inspect-brk"), script],
      {
        notificationFilter: ignoreScriptParsed,
        env: { NO_COLOR: "1" },
      },
    );

    try {
      tester.assertStderrForInspectBrk();

      tester.sendMany([
        { id: 1, method: "Runtime.enable" },
        { id: 2, method: "Debugger.enable" },
        {
          id: 3,
          method: "Debugger.setBlackboxPatterns",
          params: { patterns: ["/node_modules/|/bower_components/"] },
        },
      ]);

      await tester.expectResponse(1);
      await tester.expectResponse(2);
      await tester.expectResponse(3);
      await tester.expectNotification("Runtime.executionContextCreated");

      tester.send({ id: 4, method: "Runtime.runIfWaitingForDebugger" });
      await tester.expectResponse(4);
      await tester.expectNotification("Debugger.paused");

      tester.send({ id: 5, method: "Debugger.resume" });
      await tester.expectResponse(5);
      await tester.expectNotification("Debugger.resumed");

      // 128 iterations of pause/resume
      for (let i = 0; i < 128; i++) {
        const requestId = i + 10;

        // Expect the number on stdout
        const line = await tester.nextStdoutLine();
        assertEquals(line, String(i));

        // Collect notifications - order independent
        await tester.expectNotification("Runtime.consoleAPICalled");
        await tester.expectNotification("Debugger.paused");

        // Resume
        await tester.send({ id: requestId, method: "Debugger.resume" });
        await tester.expectResponse(requestId);
        await tester.expectNotification("Debugger.resumed");
      }

      // Close WebSocket gracefully
      await tester.close();

      assertEquals(await tester.nextStdoutLine(), "done");
    } finally {
      tester.kill();
      await tester.waitForExit();
    }
  },
});

Deno.test({
  name: "inspector_without_brk_runs_code",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const script = `${testdataPath}/inspector4.js`;
    const inspectFlag = inspectFlagWithUniquePort("--inspect");

    const command = new Deno.Command(Deno.execPath(), {
      args: ["run", inspectFlag, script],
      stdout: "piped",
      stderr: "piped",
    });

    const child = command.spawn();
    const stdoutReader = child.stdout.pipeThrough(new TextDecoderStream())
      .getReader();

    // Check that inspector actually runs code without waiting for connection
    let output = "";
    while (true) {
      const { value, done } = await stdoutReader.read();
      if (done) {
        break;
      }
      output += value;
      if (output.includes("hello")) {
        break;
      }
    }

    assertStringIncludes(output, "hello");

    child.kill();
    await child.status;
  },
});

Deno.test({
  name: "inspector_json",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const script = `${testdataPath}/inspector1.js`;
    const inspectFlag = inspectFlagWithUniquePort("--inspect");

    const command = new Deno.Command(Deno.execPath(), {
      args: ["run", inspectFlag, script],
      stderr: "piped",
    });

    const child = command.spawn();
    const stderrReader = child.stderr.pipeThrough(new TextDecoderStream())
      .getReader();

    // Extract WebSocket URL to get port
    let buffer = "";
    let wsUrl = "";
    while (true) {
      const { value, done } = await stderrReader.read();
      if (done) {
        break;
      }
      buffer += value;
      const match = buffer.match(/Debugger listening on (ws:\/\/[^\s]+)/);
      if (match) {
        wsUrl = match[1];
        break;
      }
    }

    const url = new URL(wsUrl);
    const jsonUrl = `http://${url.host}/json`;

    const response = await fetch(jsonUrl);
    assertEquals(response.status, 200);

    const data = await response.json();
    assert(Array.isArray(data));
    assert(data.length >= 1);
    assert(data[0].webSocketDebuggerUrl);

    child.kill();
    await child.status;
  },
});

Deno.test({
  name: "inspector_json_list",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const script = `${testdataPath}/inspector1.js`;
    const inspectFlag = inspectFlagWithUniquePort("--inspect");

    const command = new Deno.Command(Deno.execPath(), {
      args: ["run", inspectFlag, script],
      stderr: "piped",
    });

    const child = command.spawn();
    const stderrReader = child.stderr.pipeThrough(new TextDecoderStream())
      .getReader();

    // Extract WebSocket URL to get port
    let buffer = "";
    let wsUrl = "";
    while (true) {
      const { value, done } = await stderrReader.read();
      if (done) {
        break;
      }
      buffer += value;
      const match = buffer.match(/Debugger listening on (ws:\/\/[^\s]+)/);
      if (match) {
        wsUrl = match[1];
        break;
      }
    }

    const url = new URL(wsUrl);
    const jsonListUrl = `http://${url.host}/json/list`;

    const response = await fetch(jsonListUrl);
    assertEquals(response.status, 200);

    const data = await response.json();
    assert(Array.isArray(data));
    assert(data.length >= 1);

    child.kill();
    await child.status;
  },
});

Deno.test({
  name: "inspector_connect_non_ws",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const script = `${testdataPath}/inspector1.js`;
    const inspectFlag = inspectFlagWithUniquePort("--inspect");

    const command = new Deno.Command(Deno.execPath(), {
      args: ["run", inspectFlag, script],
      stderr: "piped",
    });

    const child = command.spawn();
    const stderrReader = child.stderr.pipeThrough(new TextDecoderStream())
      .getReader();

    // Extract WebSocket URL to get port
    let buffer = "";
    let wsUrl = "";
    while (true) {
      const { value, done } = await stderrReader.read();
      if (done) {
        break;
      }
      buffer += value;
      const match = buffer.match(/Debugger listening on (ws:\/\/[^\s]+)/);
      if (match) {
        wsUrl = match[1];
        break;
      }
    }

    const url = new URL(wsUrl);
    // Try to connect without WebSocket upgrade - should fail
    const httpUrl = `http://${url.host}${url.pathname}`;

    const response = await fetch(httpUrl);
    assertEquals(response.status, 400);

    child.kill();
    await child.status;
  },
});

Deno.test({
  name: "inspector_memory",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const script = `${testdataPath}/memory.js`;
    const tester = await InspectorTester.create(
      ["run", inspectFlagWithUniquePort("--inspect-brk"), script],
      {
        notificationFilter: ignoreScriptParsed,
        env: { RUST_BACKTRACE: "1" },
      },
    );

    try {
      await tester.assertStderrForInspectBrk();

      // Send all setup commands at once
      tester.sendMany([
        { id: 1, method: "Runtime.enable" },
        { id: 2, method: "Debugger.enable" },
        { id: 3, method: "Runtime.runIfWaitingForDebugger" },
        { id: 4, method: "HeapProfiler.enable" },
      ]);

      // Collect responses - order independent
      await tester.expectResponse(1);
      await tester.expectResponse(2);
      await tester.expectResponse(3);
      await tester.expectResponse(4);
      await tester.expectNotification("Runtime.executionContextCreated");
      await tester.expectNotification("Debugger.paused");

      // Get heap usage
      tester.send({ id: 5, method: "Runtime.getHeapUsage", params: {} });
      const heapUsage = await tester.expectResponse(5);
      const result = heapUsage.result as Record<string, number>;
      assert(result.usedSize <= result.totalSize);

      // Take heap snapshot
      tester.send({
        id: 6,
        method: "HeapProfiler.takeHeapSnapshot",
        params: {
          reportProgress: true,
          treatGlobalObjectsAsRoots: true,
          captureNumberValue: false,
        },
      });

      // Wait for snapshot to complete (collect progress notifications)
      await tester.expectResponse(6, { timeout: 30000 });
    } finally {
      tester.kill();
      await tester.waitForExit();
    }
  },
});

Deno.test({
  name: "inspector_profile",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const script = `${testdataPath}/memory.js`;
    const tester = await InspectorTester.create(
      ["run", inspectFlagWithUniquePort("--inspect-brk"), script],
      { notificationFilter: ignoreScriptParsed },
    );

    try {
      await tester.assertStderrForInspectBrk();

      // Send all setup commands at once
      tester.sendMany([
        { id: 1, method: "Runtime.enable" },
        { id: 2, method: "Debugger.enable" },
        { id: 3, method: "Runtime.runIfWaitingForDebugger" },
        { id: 4, method: "Profiler.enable" },
      ]);

      // Collect responses - order independent
      await tester.expectResponse(1);
      await tester.expectResponse(2);
      await tester.expectResponse(3);
      await tester.expectResponse(4);
      await tester.expectNotification("Runtime.executionContextCreated");
      await tester.expectNotification("Debugger.paused");

      // Start profiler
      tester.sendMany([
        {
          id: 5,
          method: "Profiler.setSamplingInterval",
          params: { interval: 100 },
        },
        { id: 6, method: "Profiler.start", params: {} },
      ]);
      await tester.expectResponse(5);
      await tester.expectResponse(6);

      // Wait a bit for sampling
      await new Promise((r) => setTimeout(r, 200));

      // Stop profiler
      tester.send({ id: 7, method: "Profiler.stop", params: {} });
      const profileResult = await tester.expectResponse(7);
      const result = profileResult.result as Record<string, unknown>;
      const profile = result.profile as Record<string, unknown>;
      assert(
        (profile.startTime as number) < (profile.endTime as number),
        "Profile startTime should be less than endTime",
      );
      assert(Array.isArray(profile.samples), "Profile should have samples");
      assert(Array.isArray(profile.nodes), "Profile should have nodes");
    } finally {
      tester.kill();
      await tester.waitForExit();
    }
  },
});

Deno.test({
  name: "inspector_multiple_workers",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const script = `${testdataPath}/multi_worker_main.js`;
    const tester = await InspectorTester.create(
      ["run", "-A", inspectFlagWithUniquePort("--inspect-brk"), script],
      { notificationFilter: ignoreScriptParsed },
    );

    try {
      await tester.assertStderrForInspectBrk();

      // Enable runtime, debugger, target auto-attach, and run
      tester.sendMany([
        { id: 1, method: "Runtime.enable" },
        { id: 2, method: "Debugger.enable" },
        {
          id: 3,
          method: "Target.setAutoAttach",
          params: {
            autoAttach: true,
            waitForDebuggerOnStart: true,
            flatten: true,
          },
        },
        { id: 4, method: "Runtime.runIfWaitingForDebugger" },
      ]);

      // Collect responses
      await tester.expectResponse(1);
      await tester.expectResponse(2);
      await tester.expectResponse(3);
      await tester.expectResponse(4);
      await tester.expectNotification("Runtime.executionContextCreated");
      await tester.expectNotification("Debugger.paused");

      // Resume to let script run and spawn workers
      tester.send({ id: 5, method: "Debugger.resume" });
      await tester.expectResponse(5);

      // Wait for worker attachments
      const worker1 = await tester.expectNotification(
        "Target.attachedToTarget",
      );
      const worker2 = await tester.expectNotification(
        "Target.attachedToTarget",
      );

      // Verify we got session IDs
      const params1 = worker1.params as Record<string, unknown>;
      const params2 = worker2.params as Record<string, unknown>;
      assert(params1.sessionId, "Worker 1 should have sessionId");
      assert(params2.sessionId, "Worker 2 should have sessionId");
    } finally {
      tester.kill();
      await tester.waitForExit();
    }
  },
});

Deno.test({
  name: "inspector_worker_target_discovery",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const script = `${testdataPath}/worker_main.js`;
    const tester = await InspectorTester.create(
      ["run", "-A", inspectFlagWithUniquePort("--inspect-brk"), script],
      { notificationFilter: ignoreScriptParsed },
    );

    try {
      await tester.assertStderrForInspectBrk();

      // Enable runtime, debugger, target discovery, and run
      tester.sendMany([
        { id: 1, method: "Runtime.enable" },
        { id: 2, method: "Debugger.enable" },
        {
          id: 3,
          method: "Target.setDiscoverTargets",
          params: { discover: true },
        },
        { id: 4, method: "Runtime.runIfWaitingForDebugger" },
      ]);

      // Collect responses
      await tester.expectResponse(1);
      await tester.expectResponse(2);
      await tester.expectResponse(3);
      await tester.expectResponse(4);
      await tester.expectNotification("Runtime.executionContextCreated");
      await tester.expectNotification("Debugger.paused");

      // Resume to let worker start
      tester.send({ id: 5, method: "Debugger.resume" });
      await tester.expectResponse(5);

      // Wait for worker target creation - the target info should contain
      // "worker" Use a custom loop to find the right target (main target is
      // created first)
      let foundWorkerTarget = false;
      const deadline = Date.now() + 10000;
      while (Date.now() < deadline) {
        try {
          const targetCreated = await tester.expectNotification(
            "Target.targetCreated",
            { timeout: 2000 },
          );
          const params = targetCreated.params as Record<string, unknown>;
          const targetInfo = params.targetInfo as Record<string, unknown>;
          if (
            targetInfo.type === "worker" ||
            JSON.stringify(targetInfo).includes("worker")
          ) {
            foundWorkerTarget = true;
            break;
          }
        } catch {
          // Timeout on individual notification, keep trying
        }
      }
      assert(foundWorkerTarget, "Should find a worker target");
    } finally {
      tester.kill();
      await tester.waitForExit();
    }
  },
});

Deno.test({
  name: "inspector_node_worker_enable",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const script = `${testdataPath}/worker_main.js`;
    const tester = await InspectorTester.create(
      ["run", "-A", inspectFlagWithUniquePort("--inspect-brk"), script],
      { notificationFilter: ignoreScriptParsed },
    );

    try {
      await tester.assertStderrForInspectBrk();

      // Test NodeWorker.enable
      tester.sendMany([
        { id: 1, method: "Runtime.enable" },
        { id: 2, method: "Debugger.enable" },
        {
          id: 3,
          method: "NodeWorker.enable",
          params: { waitForDebuggerOnStart: false },
        },
      ]);

      await tester.expectResponse(1);
      await tester.expectResponse(2);
      await tester.expectResponse(3);
      await tester.expectNotification("Runtime.executionContextCreated");

      // Test NodeWorker.sendMessageToWorker
      tester.send({
        id: 4,
        method: "NodeWorker.sendMessageToWorker",
        params: {
          sessionId: "nonexistent",
          message: '{"id":1,"method":"Runtime.enable"}',
        },
      });
      await tester.expectResponse(4);
    } finally {
      tester.kill();
      await tester.waitForExit();
    }
  },
});

Deno.test({
  name: "inspector_runtime_evaluate_does_not_crash",
  permissions: { run: true, read: true, net: true, env: true, write: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const tester = await InspectorTester.create(
      ["repl", "--allow-read", inspectFlagWithUniquePort("--inspect")],
      {
        notificationFilter: ignoreScriptParsed,
        env: { RUST_BACKTRACE: "1" },
      },
    );

    try {
      await tester.assertStderrForInspect();

      // Read REPL banner
      const banner = await tester.nextStdoutLine();
      assert(banner.startsWith("Deno"), `Expected Deno banner, got: ${banner}`);
      const exitMsg = await tester.nextStdoutLine();
      assertEquals(exitMsg, "exit using ctrl+d, ctrl+c, or close()");

      // "Debugger session started." on stderr
      const sessionLine = await tester.nextStderrLine();
      assertEquals(sessionLine, "Debugger session started.");

      // Enable runtime and debugger
      tester.sendMany([
        { id: 1, method: "Runtime.enable" },
        { id: 2, method: "Debugger.enable" },
      ]);

      await tester.expectResponse(1, { prefixMatch: '{"id":1,"result":{}}' });
      await tester.expectResponse(2, {
        prefixMatch: '{"id":2,"result":{"debuggerId":',
      });
      await tester.expectNotification("Runtime.executionContextCreated");

      // Compile script
      tester.send({
        id: 3,
        method: "Runtime.compileScript",
        params: {
          expression: "Deno.cwd()",
          sourceURL: "",
          persistScript: false,
          executionContextId: 1,
        },
      });
      await tester.expectResponse(3, { prefixMatch: '{"id":3,"result":{}}' });

      // Evaluate expression
      tester.send({
        id: 4,
        method: "Runtime.evaluate",
        params: {
          expression: "Deno.cwd()",
          objectGroup: "console",
          includeCommandLineAPI: true,
          silent: false,
          contextId: 1,
          returnByValue: true,
          generatePreview: true,
          userGesture: true,
          awaitPromise: false,
          replMode: true,
        },
      });
      await tester.expectResponse(4, {
        prefixMatch: '{"id":4,"result":{"result":{"type":"string","value":"',
      });

      // Evaluate console.error
      tester.send({
        id: 5,
        method: "Runtime.evaluate",
        params: {
          expression: "console.error('done');",
          objectGroup: "console",
          includeCommandLineAPI: true,
          silent: false,
          contextId: 1,
          returnByValue: true,
          generatePreview: true,
          userGesture: true,
          awaitPromise: false,
          replMode: true,
        },
      });
      await tester.expectResponse(5, {
        prefixMatch: '{"id":5,"result":{"result":{"type":"undefined"}}}',
      });
      await tester.expectNotification("Runtime.consoleAPICalled");

      // Check for "done" on stderr
      const doneLine = await tester.nextStderrLine();
      assertEquals(doneLine, "done");

      // Close stdin to end REPL
      await tester.stdin.close();
    } finally {
      tester.kill();
      await tester.waitForExit();
    }
  },
});

Deno.test({
  name: "inspector_break_on_first_line_in_test",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    // Skip on Windows - deno test --inspect-brk exits before outputting
    // debugger URL
    if (Deno.build.os === "windows") {
      return;
    }

    const script = `${testdataPath}/inspector_test.js`;
    const tester = await InspectorTester.create(
      ["test", inspectFlagWithUniquePort("--inspect-brk"), script],
      {
        notificationFilter: ignoreScriptParsed,
        env: { NO_COLOR: "1" },
      },
    );

    try {
      await tester.assertStderrForInspectBrk();

      // Enable runtime and debugger
      tester.sendMany([
        { id: 1, method: "Runtime.enable" },
        { id: 2, method: "Debugger.enable" },
      ]);

      await tester.expectResponse(1, { prefixMatch: '{"id":1,"result":{}}' });
      await tester.expectResponse(2, {
        prefixMatch: '{"id":2,"result":{"debuggerId":',
      });
      await tester.expectNotification("Runtime.executionContextCreated");

      // Run and pause
      tester.send({ id: 3, method: "Runtime.runIfWaitingForDebugger" });
      await tester.expectResponse(3, { prefixMatch: '{"id":3,"result":{}}' });
      await tester.expectNotification("Debugger.paused");

      // Evaluate while paused
      tester.send({
        id: 4,
        method: "Runtime.evaluate",
        params: {
          expression: "1 + 1",
          contextId: 1,
          includeCommandLineAPI: true,
          silent: false,
          returnByValue: true,
        },
      });
      await tester.expectResponse(4, {
        prefixMatch:
          '{"id":4,"result":{"result":{"type":"number","value":2,"description":"2"}}}',
      });

      // Resume
      tester.send({ id: 5, method: "Debugger.resume" });
      await tester.expectResponse(5, { prefixMatch: '{"id":5,"result":{}}' });

      // Check test output
      const line1 = await tester.nextStdoutLine();
      assert(
        line1.includes("running 1 test from"),
        `Expected test start, got: ${line1}`,
      );
      const line2 = await tester.nextStdoutLine();
      assert(line2.includes("basic test ... ok"), `Expected ok, got: ${line2}`);
    } finally {
      tester.kill();
      await tester.waitForExit();
    }
  },
});

Deno.test({
  name: "inspector_with_ts_files",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const script = `${testdataPath}/test.ts`;

    // Custom filter that allows testdata/inspector scriptParsed notifications
    function notificationFilter(msg: CDPMessage): boolean {
      if (msg.method === "Debugger.scriptParsed") {
        const json = JSON.stringify(msg);
        return json.includes("testdata/inspector");
      }
      return true;
    }

    const tester = await InspectorTester.create(
      [
        "run",
        "--check",
        inspectFlagWithUniquePort("--inspect-brk"),
        script,
      ],
      { notificationFilter },
    );

    try {
      await tester.assertStderrForInspectBrk();

      // "Debugger session started." on stderr - may have other output before it
      // (e.g., "Check ..." from --check flag on Linux)
      let sessionLine = "";
      const sessionDeadline = Date.now() + 10000;
      while (Date.now() < sessionDeadline) {
        sessionLine = await tester.nextStderrLine();
        if (sessionLine === "Debugger session started.") {
          break;
        }
      }
      assertEquals(sessionLine, "Debugger session started.");

      // Enable runtime and debugger
      tester.sendMany([
        { id: 1, method: "Runtime.enable" },
        { id: 2, method: "Debugger.enable" },
      ]);

      await tester.expectResponse(1, { prefixMatch: '{"id":1,"result":{}}' });
      await tester.expectNotification("Runtime.executionContextCreated");

      // Collect scriptParsed messages for test files until we have all 3
      const scripts: { url: string; scriptId: string }[] = [];
      const deadline = Date.now() + 30000;
      let debuggerResponse: CDPMessage | null = null;

      while (scripts.length < 3 && Date.now() < deadline) {
        // Try to get either a scriptParsed or the debugger response
        try {
          const notification = await tester.expectNotification(
            "Debugger.scriptParsed",
            { timeout: 2000 },
          );
          const params = notification.params as Record<string, unknown>;
          const url = params.url as string;
          if (url && url.includes("testdata/inspector")) {
            scripts.push({ url, scriptId: params.scriptId as string });
          }
        } catch {
          // No scriptParsed notification available, check for debugger response
        }

        if (!debuggerResponse) {
          try {
            debuggerResponse = await tester.expectResponse(2, { timeout: 100 });
          } catch {
            // Not ready yet
          }
        }
      }

      // Make sure we got the debugger response
      if (!debuggerResponse) {
        debuggerResponse = await tester.expectResponse(2);
      }

      // Find the script IDs for our files
      const testTs = scripts.find((s) => s.url.includes("test.ts"));
      const fooTs = scripts.find((s) => s.url.includes("foo.ts"));
      const barJs = scripts.find((s) => s.url.includes("bar.js"));

      assert(testTs, "Should have test.ts");
      assert(fooTs, "Should have foo.ts");
      assert(barJs, "Should have bar.js");

      // Run and pause
      tester.send({ id: 3, method: "Runtime.runIfWaitingForDebugger" });
      await tester.expectResponse(3);
      await tester.expectNotification("Debugger.paused");

      // Get script sources
      tester.sendMany([
        {
          id: 4,
          method: "Debugger.getScriptSource",
          params: { scriptId: testTs.scriptId },
        },
        {
          id: 5,
          method: "Debugger.getScriptSource",
          params: { scriptId: fooTs.scriptId },
        },
        {
          id: 6,
          method: "Debugger.getScriptSource",
          params: { scriptId: barJs.scriptId },
        },
      ]);

      // Verify test.ts source
      const source1 = await tester.expectResponse(4);
      const result1 = source1.result as Record<string, string>;
      assert(
        result1.scriptSource.includes('import { foo } from "./foo.ts"'),
        "test.ts should have foo import",
      );

      // Verify foo.ts source
      const source2 = await tester.expectResponse(5);
      const result2 = source2.result as Record<string, string>;
      assert(
        result2.scriptSource.includes("class Foo"),
        "foo.ts should have class Foo",
      );

      // Verify bar.js source
      const source3 = await tester.expectResponse(6);
      const result3 = source3.result as Record<string, string>;
      assert(
        result3.scriptSource.includes("export function bar"),
        "bar.js should have bar function",
      );

      // Resume
      tester.send({ id: 7, method: "Debugger.resume" });
      await tester.expectResponse(7);

      // Check output
      assertEquals(await tester.nextStdoutLine(), "hello");
      assertEquals(await tester.nextStdoutLine(), "world");
    } finally {
      tester.kill();
      await tester.waitForExit();
    }
  },
});

Deno.test({
  name: "inspector_wait",
  permissions: { run: true, read: true, net: true, env: true, write: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const script = `${testdataPath}/inspect_wait.js`;

    // Create a temp directory for the test
    const tempDir = await Deno.makeTempDir();
    const helloPath = `${tempDir}/hello.txt`;
    const inspectFlag = inspectFlagWithUniquePort("--inspect-wait");

    try {
      // Spawn process but DON'T connect to WebSocket yet
      const command = new Deno.Command(Deno.execPath(), {
        args: ["run", "-A", inspectFlag, script],
        stdin: "piped",
        stdout: "piped",
        stderr: "piped",
        cwd: tempDir,
      });

      const child = command.spawn();
      const stderrReader = child.stderr.pipeThrough(new TextDecoderStream())
        .getReader();

      // Sleep a bit and verify script hasn't run yet
      await new Promise((r) => setTimeout(r, 300));

      // The file shouldn't exist yet since we're waiting for debugger
      let fileExists = false;
      try {
        await Deno.stat(helloPath);
        fileExists = true;
      } catch {
        // Expected - file shouldn't exist yet
      }
      assert(!fileExists, "File should not exist before debugger connects");

      // Extract WebSocket URL from stderr
      let buffer = "";
      let wsUrl = "";
      while (true) {
        const { value, done } = await stderrReader.read();
        if (done) {
          break;
        }
        buffer += value;
        const match = buffer.match(/Debugger listening on (ws:\/\/[^\s]+)/);
        if (match) {
          wsUrl = match[1];
          break;
        }
      }

      // Connect to WebSocket
      const socket = new WebSocket(wsUrl);
      await new Promise<void>((resolve, reject) => {
        socket.onopen = () => resolve();
        socket.onerror = (e) => reject(e);
      });

      // Enable runtime and debugger
      let msgId = 1;
      const send = (msg: Record<string, unknown>) => {
        socket.send(JSON.stringify(msg));
      };
      const messages: string[] = [];
      socket.onmessage = (e) => {
        messages.push(e.data as string);
      };

      send({ id: msgId++, method: "Runtime.enable" });
      send({ id: msgId++, method: "Debugger.enable" });

      // Wait for responses
      await new Promise((r) => setTimeout(r, 500));

      // Run
      send({ id: msgId++, method: "Runtime.runIfWaitingForDebugger" });

      // Wait for script to run
      await new Promise((r) => setTimeout(r, 500));

      // Read remaining stderr
      let stderrContent = buffer;
      const deadline = Date.now() + 5000;
      while (Date.now() < deadline) {
        const result = await Promise.race([
          stderrReader.read(),
          new Promise<{ value: undefined; done: true }>(
            (r) => setTimeout(() => r({ value: undefined, done: true }), 500),
          ),
        ]);
        if (result.done || !result.value) {
          break;
        }
        stderrContent += result.value;
        if (stderrContent.includes("did run")) {
          break;
        }
      }

      // Verify "did run" appeared on stderr
      assert(
        stderrContent.includes("did run"),
        `Expected 'did run' in stderr: ${stderrContent}`,
      );

      // File should exist now
      try {
        await Deno.stat(helloPath);
        fileExists = true;
      } catch {
        fileExists = false;
      }
      assert(fileExists, "File should exist after script runs");

      socket.close();
      child.kill();
      await child.status;
    } finally {
      // Clean up temp dir
      try {
        await Deno.remove(tempDir, { recursive: true });
      } catch {
        // Ignore cleanup errors
      }
    }
  },
});

Deno.test({
  name: "inspector_node_runtime_api_url",
  permissions: { run: true, read: true, net: true, env: true },
  sanitizeOps: false,
  sanitizeResources: false,
  async fn() {
    const script = `${testdataPath}/node/url.js`;
    const inspectFlag = inspectFlagWithUniquePort("--inspect");

    const command = new Deno.Command(Deno.execPath(), {
      args: ["run", "--allow-sys", inspectFlag, script],
      stdout: "piped",
      stderr: "piped",
    });

    const output = await command.output();
    const stderr = new TextDecoder().decode(output.stderr);
    const stdout = new TextDecoder().decode(output.stdout);

    // First line of stderr should be "Debugger listening on ws://..."
    const firstLine = stderr.split("\n")[0];
    assert(
      firstLine.startsWith("Debugger listening on "),
      `Expected debugger URL, got: ${firstLine}`,
    );

    const expectedUrl = firstLine.slice("Debugger listening on ".length);
    const actualUrl = stdout.trim();

    assertEquals(
      actualUrl,
      expectedUrl,
      "inspector.url() should return the same URL as stderr",
    );
  },
});
