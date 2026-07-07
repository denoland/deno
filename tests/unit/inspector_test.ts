// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals, assertStringIncludes } from "./test_util.ts";
import { fromFileUrl } from "@std/path";

const testdataPath = fromFileUrl(
  new URL("../testdata/inspector/", import.meta.url),
);

interface CDPMessage {
  id?: number;
  method?: string;
  sessionId?: string;
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

function ignoreScriptParsed(msg: CDPMessage): boolean {
  return msg.method !== "Debugger.scriptParsed";
}

async function extractWsUrl(
  reader: ReadableStreamDefaultReader<string>,
): Promise<string> {
  let buffer = "";
  while (true) {
    const { value, done } = await reader.read();
    if (done) throw new Error("Stream closed before WebSocket URL found");
    buffer += value;
    const match = buffer.match(/Debugger listening on (ws:\/\/[^\s]+)/);
    if (match) return match[1];
  }
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

  async expectNotificationMatching(
    predicate: (msg: CDPMessage) => boolean,
    description: string,
    options?: { timeout?: number },
  ): Promise<CDPMessage> {
    const timeoutMs = options?.timeout ?? this.timeout;
    const deadline = Date.now() + timeoutMs;

    while (Date.now() < deadline) {
      const idx = this.notificationBuffer.findIndex(predicate);
      if (idx !== -1) {
        const [msg] = this.notificationBuffer.splice(idx, 1);
        return msg;
      }

      if (this.socketClosed) {
        throw new Error(
          `Socket closed while waiting for notification ${description}`,
        );
      }

      try {
        await this.waitForMessage(Math.min(1000, deadline - Date.now()));
      } catch {
        // continue
      }
    }

    throw new Error(`Timeout waiting for notification ${description}`);
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
    try {
      await this.child.stdin.close();
    } catch {
      // may already be closed
    }
    try {
      await this.stderrReader.cancel();
    } catch {
      // may already be closed
    }
    try {
      await this.stdoutReader.cancel();
    } catch {
      // may already be closed
    }
    return await this.child.status;
  }

  get stdin(): WritableStream<Uint8Array> {
    return this.child.stdin;
  }
}

Deno.test("inspector_connect", async () => {
  const script = `${testdataPath}/inspector1.js`;
  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect=0", script],
    { notificationFilter: ignoreScriptParsed },
  );

  try {
    tester.send({ id: 1, method: "Runtime.enable" });
    await tester.expectResponse(1);
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

Deno.test("inspector_break_on_first_line", async () => {
  const script = `${testdataPath}/inspector2.js`;
  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect-brk=0", script],
    { notificationFilter: ignoreScriptParsed },
  );

  try {
    await tester.assertStderrForInspectBrk();

    tester.sendMany([
      { id: 1, method: "Runtime.enable" },
      { id: 2, method: "Debugger.enable" },
    ]);

    await tester.expectResponse(1);
    await tester.expectResponse(2);
    await tester.expectNotification("Runtime.executionContextCreated");

    tester.send({ id: 3, method: "Runtime.runIfWaitingForDebugger" });
    await tester.expectResponse(3);
    await tester.expectNotification("Debugger.paused");

    tester.send({
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
    });
    await tester.expectResponse(4);

    const inspectorOutput = await tester.nextStdoutLine();
    assertEquals(inspectorOutput, "hello from the inspector");

    tester.send({ id: 5, method: "Debugger.resume" });
    await tester.expectResponse(5);

    const scriptOutput = await tester.nextStdoutLine();
    assertEquals(scriptOutput, "hello from the script");
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

Deno.test("inspector_pause", async () => {
  const script = `${testdataPath}/inspector1.js`;
  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect=0", script],
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
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

Deno.test("inspector_does_not_hang", async () => {
  const script = `${testdataPath}/inspector3.js`;
  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect-brk=0", script],
    {
      notificationFilter: ignoreScriptParsed,
      env: { NO_COLOR: "1" },
    },
  );

  try {
    await tester.assertStderrForInspectBrk();

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

    for (let i = 0; i < 128; i++) {
      const requestId = i + 10;
      const line = await tester.nextStdoutLine();
      assertEquals(line, String(i));

      await tester.expectNotification("Runtime.consoleAPICalled");
      await tester.expectNotification("Debugger.paused");

      tester.send({ id: requestId, method: "Debugger.resume" });
      await tester.expectResponse(requestId);
      await tester.expectNotification("Debugger.resumed");
    }

    await tester.close();
    assertEquals(await tester.nextStdoutLine(), "done");
  } finally {
    tester.kill();
    await tester.waitForExit();
  }
});

Deno.test("inspector_without_brk_runs_code", async () => {
  const script = `${testdataPath}/inspector4.js`;

  const command = new Deno.Command(Deno.execPath(), {
    args: ["run", "-A", "--inspect=0", script],
    stdout: "piped",
    stderr: "piped",
  });

  const child = command.spawn();
  const stdoutReader = child.stdout.pipeThrough(new TextDecoderStream())
    .getReader();
  const stderrReader = child.stderr.pipeThrough(new TextDecoderStream())
    .getReader();

  let output = "";
  while (true) {
    const { value, done } = await stdoutReader.read();
    if (done) break;
    output += value;
    if (output.includes("hello")) break;
  }

  assertStringIncludes(output, "hello");

  await stdoutReader.cancel();
  await stderrReader.cancel();
  child.kill();
  await child.status;
});

Deno.test("inspector_json", async () => {
  const script = `${testdataPath}/inspector1.js`;

  const command = new Deno.Command(Deno.execPath(), {
    args: ["run", "-A", "--inspect=0", script],
    stderr: "piped",
  });

  const child = command.spawn();
  const stderrReader = child.stderr.pipeThrough(new TextDecoderStream())
    .getReader();

  const wsUrl = await extractWsUrl(stderrReader);
  const url = new URL(wsUrl);

  const jsonResponse = await fetch(`http://${url.host}/json`);
  assertEquals(jsonResponse.status, 200);
  const jsonData = await jsonResponse.json();
  assert(Array.isArray(jsonData));
  assert(jsonData.length >= 1);
  assert(jsonData[0].webSocketDebuggerUrl);

  const listResponse = await fetch(`http://${url.host}/json/list`);
  assertEquals(listResponse.status, 200);
  const listData = await listResponse.json();
  assert(Array.isArray(listData));
  assert(listData.length >= 1);

  await stderrReader.cancel();
  child.kill();
  await child.status;
});

Deno.test("inspector_connect_non_ws", async () => {
  const script = `${testdataPath}/inspector1.js`;

  const command = new Deno.Command(Deno.execPath(), {
    args: ["run", "-A", "--inspect=0", script],
    stderr: "piped",
  });

  const child = command.spawn();
  const stderrReader = child.stderr.pipeThrough(new TextDecoderStream())
    .getReader();

  const wsUrl = await extractWsUrl(stderrReader);
  const url = new URL(wsUrl);
  const httpUrl = `http://${url.host}${url.pathname}`;
  const response = await fetch(httpUrl);
  assertEquals(response.status, 400);
  await response.body?.cancel();

  await stderrReader.cancel();
  child.kill();
  await child.status;
});

Deno.test("inspector_memory", async () => {
  const script = `${testdataPath}/memory.js`;
  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect-brk=0", script],
    {
      notificationFilter: ignoreScriptParsed,
      env: { RUST_BACKTRACE: "1" },
    },
  );

  try {
    await tester.assertStderrForInspectBrk();

    tester.sendMany([
      { id: 1, method: "Runtime.enable" },
      { id: 2, method: "Debugger.enable" },
      { id: 3, method: "Runtime.runIfWaitingForDebugger" },
      { id: 4, method: "HeapProfiler.enable" },
    ]);

    await tester.expectResponse(1);
    await tester.expectResponse(2);
    await tester.expectResponse(3);
    await tester.expectResponse(4);
    await tester.expectNotification("Runtime.executionContextCreated");
    await tester.expectNotification("Debugger.paused");

    tester.send({ id: 5, method: "Runtime.getHeapUsage", params: {} });
    const heapUsage = await tester.expectResponse(5);
    const result = heapUsage.result as Record<string, number>;
    assert(result.usedSize <= result.totalSize);

    tester.send({
      id: 6,
      method: "HeapProfiler.takeHeapSnapshot",
      params: {
        reportProgress: true,
        treatGlobalObjectsAsRoots: true,
        captureNumberValue: false,
      },
    });

    await tester.expectResponse(6, { timeout: 30000 });
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

Deno.test("inspector_profile", async () => {
  const script = `${testdataPath}/memory.js`;
  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect-brk=0", script],
    { notificationFilter: ignoreScriptParsed },
  );

  try {
    await tester.assertStderrForInspectBrk();

    tester.sendMany([
      { id: 1, method: "Runtime.enable" },
      { id: 2, method: "Debugger.enable" },
      { id: 3, method: "Runtime.runIfWaitingForDebugger" },
      { id: 4, method: "Profiler.enable" },
    ]);

    await tester.expectResponse(1);
    await tester.expectResponse(2);
    await tester.expectResponse(3);
    await tester.expectResponse(4);
    await tester.expectNotification("Runtime.executionContextCreated");
    await tester.expectNotification("Debugger.paused");

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

    await new Promise((r) => setTimeout(r, 200));

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
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

// Regression test for https://github.com/denoland/deno/issues/21620: an idle
// process (one that only awaits timers) must report its wait time as the
// "(idle)" node in a CPU profile, not as "(program)"/"Scripting" which made
// DevTools show ~100% CPU usage for a process doing nothing. This relies on
// the event loop telling V8's CPU profiler it is idle (Isolate::SetIdle)
// whenever it parks waiting for external events.
Deno.test("inspector_profile_idle", async () => {
  const script = `${testdataPath}/idle.js`;
  // Use --inspect (not --inspect-brk) so the program runs straight into its
  // idle loop, matching how the issue is reproduced (`deno run --inspect`).
  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect=0", script],
    { notificationFilter: ignoreScriptParsed },
  );

  try {
    tester.sendMany([
      { id: 1, method: "Runtime.enable" },
      { id: 2, method: "Profiler.enable" },
      {
        id: 3,
        method: "Profiler.setSamplingInterval",
        params: { interval: 100 },
      },
      { id: 4, method: "Profiler.start", params: {} },
    ]);
    await tester.expectResponse(1);
    await tester.expectResponse(2);
    await tester.expectResponse(3);
    await tester.expectResponse(4);

    // Let the (idle) process run for a while so plenty of idle samples land.
    await new Promise((r) => setTimeout(r, 1000));

    tester.send({ id: 5, method: "Profiler.stop", params: {} });
    const profileResult = await tester.expectResponse(5);
    const result = profileResult.result as Record<string, unknown>;
    const profile = result.profile as {
      nodes: Array<
        { hitCount?: number; callFrame: { functionName: string } }
      >;
    };

    const hitsFor = (name: string) =>
      profile.nodes
        .filter((n) => n.callFrame.functionName === name)
        .reduce((sum, n) => sum + (n.hitCount ?? 0), 0);

    const idleHits = hitsFor("(idle)");
    const programHits = hitsFor("(program)");

    // The wait time must be attributed to "(idle)", not "(program)". Without the
    // SetIdle notification these samples all land in "(program)".
    assert(
      idleHits > 0,
      `Expected idle samples in profile, got idle=${idleHits} program=${programHits}`,
    );
    assert(
      idleHits > programHits,
      `Expected idle samples to dominate, got idle=${idleHits} program=${programHits}`,
    );
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

Deno.test("inspector_multiple_workers", async () => {
  const script = `${testdataPath}/multi_worker_main.js`;
  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect-brk=0", script],
    { notificationFilter: ignoreScriptParsed },
  );

  try {
    await tester.assertStderrForInspectBrk();

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

    await tester.expectResponse(1);
    await tester.expectResponse(2);
    await tester.expectResponse(3);
    await tester.expectResponse(4);
    await tester.expectNotification("Runtime.executionContextCreated");
    await tester.expectNotification("Debugger.paused");

    tester.send({ id: 5, method: "Debugger.resume" });
    await tester.expectResponse(5);

    const worker1 = await tester.expectNotification("Target.attachedToTarget");
    const worker2 = await tester.expectNotification("Target.attachedToTarget");

    const params1 = worker1.params as Record<string, unknown>;
    const params2 = worker2.params as Record<string, unknown>;
    assert(params1.sessionId, "Worker 1 should have sessionId");
    assert(params2.sessionId, "Worker 2 should have sessionId");
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

Deno.test("inspector_worker_target_discovery", async () => {
  const script = `${testdataPath}/worker_main.js`;
  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect-brk=0", script],
    { notificationFilter: ignoreScriptParsed },
  );

  try {
    await tester.assertStderrForInspectBrk();

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

    await tester.expectResponse(1);
    await tester.expectResponse(2);
    await tester.expectResponse(3);
    await tester.expectResponse(4);
    await tester.expectNotification("Runtime.executionContextCreated");
    await tester.expectNotification("Debugger.paused");

    tester.send({ id: 5, method: "Debugger.resume" });
    await tester.expectResponse(5);

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
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

Deno.test("inspector_worker_target_get_targets_and_attach", async () => {
  const script = `${testdataPath}/worker_main.js`;
  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect-brk=0", script],
    { notificationFilter: ignoreScriptParsed },
  );

  try {
    await tester.assertStderrForInspectBrk();

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

    await tester.expectResponse(1);
    await tester.expectResponse(2);
    await tester.expectResponse(3);
    await tester.expectResponse(4);
    await tester.expectNotification("Runtime.executionContextCreated");
    await tester.expectNotification("Debugger.paused");

    tester.send({ id: 5, method: "Debugger.resume" });
    await tester.expectResponse(5);

    const targetCreated = await tester.expectNotification(
      "Target.targetCreated",
    );
    const targetCreatedParams = targetCreated.params as Record<string, unknown>;
    const targetInfo = targetCreatedParams.targetInfo as Record<
      string,
      unknown
    >;
    const targetId = targetInfo.targetId as string;
    assert(targetId, "targetCreated should include targetId");

    tester.send({ id: 6, method: "Target.getTargets" });
    const targetsResponse = await tester.expectResponse(6);
    const targetsResult = targetsResponse.result as {
      targetInfos: Array<Record<string, unknown>>;
    };
    assert(
      targetsResult.targetInfos.some((info) => info.targetId === targetId),
      "getTargets should include discovered worker target",
    );

    tester.send({
      id: 7,
      method: "Target.attachToTarget",
      params: { targetId, flatten: true },
    });
    const attachedResponse = await tester.expectResponse(7);
    const attachedResult = attachedResponse.result as Record<string, unknown>;
    const sessionId = attachedResult.sessionId as string;
    assert(sessionId, "attachToTarget should return sessionId");

    tester.send({ id: 8, sessionId, method: "Runtime.enable" });
    await tester.expectResponse(8);
    await tester.expectNotificationMatching(
      (msg) =>
        msg.sessionId === sessionId &&
        msg.method === "Runtime.executionContextCreated",
      "worker Runtime.executionContextCreated after attachToTarget",
    );
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

Deno.test("inspector_worker_debugger_statement_not_blackboxed", async () => {
  const tempDir = await Deno.makeTempDir();
  const mainScript = `${tempDir}/main.js`;
  const workerScript = `${tempDir}/worker.js`;
  await Deno.writeTextFile(
    mainScript,
    `
globalThis.worker = new Worker(new URL("./worker.js", import.meta.url).href, {
  type: "module",
});
globalThis.worker.onmessage = (e) => {
  console.log("Main received:", e.data);
};
setInterval(() => {}, 1000);
`,
  );
  await Deno.writeTextFile(
    workerScript,
    `
self.onmessage = (e) => {
  console.log("Worker received:", e.data);
  debugger;
  console.log("after debugger");
};
self.postMessage("ready");
setInterval(() => {}, 1000);
`,
  );

  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect-brk=0", mainScript],
    { notificationFilter: ignoreScriptParsed },
  );

  try {
    await tester.assertStderrForInspectBrk();

    tester.sendMany([
      { id: 1, method: "Runtime.enable" },
      { id: 2, method: "Debugger.enable" },
      {
        id: 3,
        method: "Target.setAutoAttach",
        params: {
          autoAttach: true,
          waitForDebuggerOnStart: false,
          flatten: true,
        },
      },
      { id: 4, method: "Runtime.runIfWaitingForDebugger" },
    ]);

    await tester.expectResponse(1);
    await tester.expectResponse(2);
    await tester.expectResponse(3);
    await tester.expectResponse(4);
    await tester.expectNotification("Runtime.executionContextCreated");
    await tester.expectNotification("Debugger.paused");

    tester.send({ id: 5, method: "Debugger.resume" });
    await tester.expectResponse(5);

    const attached = await tester.expectNotification("Target.attachedToTarget");
    const attachedParams = attached.params as Record<string, unknown>;
    const sessionId = attachedParams.sessionId as string;
    assert(sessionId, "attachedToTarget should include sessionId");

    tester.sendMany([
      { id: 6, sessionId, method: "Runtime.enable" },
      { id: 7, sessionId, method: "Debugger.enable" },
      {
        id: 8,
        sessionId,
        method: "Debugger.setBlackboxPatterns",
        params: { patterns: ["/node_modules/|^node:"], skipAnonymous: false },
      },
    ]);
    await tester.expectResponse(6);
    await tester.expectResponse(7);
    await tester.expectResponse(8);

    const contextCreated = await tester.expectNotificationMatching(
      (msg) =>
        msg.sessionId === sessionId &&
        msg.method === "Runtime.executionContextCreated",
      "worker Runtime.executionContextCreated",
    );
    const context = (contextCreated.params as {
      context: {
        name: string;
        auxData: { isDefault: boolean; type: string };
      };
    }).context;
    assertEquals(context.name, "worker [1]");
    assertEquals(context.auxData, { isDefault: true, type: "worker" });

    assertEquals(await tester.nextStdoutLine(), "Main received: ready");
    tester.send({
      id: 9,
      method: "Runtime.evaluate",
      params: {
        expression: 'globalThis.worker.postMessage("go")',
        returnByValue: true,
      },
    });
    await tester.expectResponse(9);

    assertEquals(await tester.nextStdoutLine(), "Worker received: go");
    await tester.expectNotificationMatching(
      (msg) => msg.sessionId === sessionId && msg.method === "Debugger.paused",
      "worker Debugger.paused",
    );

    tester.send({ id: 10, sessionId, method: "Debugger.resume" });
    await tester.expectResponse(10);
    assertEquals(await tester.nextStdoutLine(), "after debugger");
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
    await Deno.remove(tempDir, { recursive: true });
  }
});

Deno.test("inspector_worker_page_wait_for_debugger", async () => {
  const tempDir = await Deno.makeTempDir();
  const mainScript = `${tempDir}/main.js`;
  const workerScript = `${tempDir}/worker.js`;
  await Deno.writeTextFile(
    mainScript,
    `
new Worker(new URL("./worker.js", import.meta.url).href, {
  type: "module",
});
setInterval(() => {}, 1000);
`,
  );
  await Deno.writeTextFile(
    workerScript,
    `
console.log("worker before debugger");
debugger;
console.log("worker after debugger");
setInterval(() => {}, 1000);
`,
  );

  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect-brk=0", mainScript],
    { notificationFilter: ignoreScriptParsed },
  );

  try {
    await tester.assertStderrForInspectBrk();

    tester.sendMany([
      { id: 1, method: "Runtime.enable" },
      { id: 2, method: "Debugger.enable" },
      {
        id: 3,
        method: "Target.setAutoAttach",
        params: {
          autoAttach: true,
          waitForDebuggerOnStart: false,
          flatten: true,
        },
      },
      { id: 4, method: "Runtime.runIfWaitingForDebugger" },
    ]);

    await tester.expectResponse(1);
    await tester.expectResponse(2);
    await tester.expectResponse(3);
    await tester.expectResponse(4);
    await tester.expectNotification("Runtime.executionContextCreated");
    await tester.expectNotification("Debugger.paused");

    tester.send({ id: 5, method: "Debugger.resume" });
    await tester.expectResponse(5);

    const attached = await tester.expectNotification("Target.attachedToTarget");
    const attachedParams = attached.params as Record<string, unknown>;
    assertEquals(attachedParams.waitingForDebugger, false);
    const sessionId = attachedParams.sessionId as string;
    assert(sessionId, "attachedToTarget should include sessionId");

    tester.sendMany([
      { id: 6, sessionId, method: "Page.waitForDebugger" },
      { id: 7, sessionId, method: "Runtime.enable" },
      { id: 8, sessionId, method: "Debugger.enable" },
    ]);
    const waitResponse = await tester.expectResponse(6);
    assertEquals(waitResponse.error, undefined);
    await tester.expectResponse(7);
    await tester.expectResponse(8);

    const contextCreated = await tester.expectNotificationMatching(
      (msg) =>
        msg.sessionId === sessionId &&
        msg.method === "Runtime.executionContextCreated",
      "worker Runtime.executionContextCreated",
    );
    const context = (contextCreated.params as {
      context: {
        auxData: { isDefault: boolean; type: string };
      };
    }).context;
    assertEquals(context.auxData, { isDefault: true, type: "worker" });

    tester.send({
      id: 9,
      sessionId,
      method: "Runtime.runIfWaitingForDebugger",
    });
    await tester.expectResponse(9);

    assertEquals(await tester.nextStdoutLine(), "worker before debugger");
    await tester.expectNotificationMatching(
      (msg) => msg.sessionId === sessionId && msg.method === "Debugger.paused",
      "worker Debugger.paused",
    );

    tester.send({ id: 10, sessionId, method: "Debugger.resume" });
    await tester.expectResponse(10);
    assertEquals(await tester.nextStdoutLine(), "worker after debugger");
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
    await Deno.remove(tempDir, { recursive: true });
  }
});

Deno.test("inspector_worker_wait_for_debugger_on_start", async () => {
  const tempDir = await Deno.makeTempDir();
  const mainScript = `${tempDir}/main.js`;
  const workerScript = `${tempDir}/worker.js`;
  await Deno.writeTextFile(
    mainScript,
    `
const worker = new Worker(new URL("./worker.js", import.meta.url).href, {
  type: "module",
});
worker.postMessage("start");
setInterval(() => {}, 1000);
`,
  );
  await Deno.writeTextFile(
    workerScript,
    `
self.onmessage = (e) => {
  console.log("Worker received:", e.data);
  debugger;
  console.log("after debugger");
};
setInterval(() => {}, 1000);
`,
  );

  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect-brk=0", mainScript],
    { notificationFilter: ignoreScriptParsed },
  );

  try {
    await tester.assertStderrForInspectBrk();

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

    await tester.expectResponse(1);
    await tester.expectResponse(2);
    await tester.expectResponse(3);
    await tester.expectResponse(4);
    await tester.expectNotification("Runtime.executionContextCreated");
    await tester.expectNotification("Debugger.paused");

    tester.send({ id: 5, method: "Debugger.resume" });
    await tester.expectResponse(5);

    const attached = await tester.expectNotification("Target.attachedToTarget");
    const attachedParams = attached.params as Record<string, unknown>;
    assertEquals(attachedParams.waitingForDebugger, true);
    const sessionId = attachedParams.sessionId as string;
    assert(sessionId, "attachedToTarget should include sessionId");

    tester.sendMany([
      { id: 6, sessionId, method: "Runtime.enable" },
      { id: 7, sessionId, method: "Debugger.enable" },
      {
        id: 8,
        sessionId,
        method: "Runtime.runIfWaitingForDebugger",
      },
    ]);
    await tester.expectResponse(6);
    await tester.expectResponse(7);
    await tester.expectResponse(8);

    const contextCreated = await tester.expectNotificationMatching(
      (msg) =>
        msg.sessionId === sessionId &&
        msg.method === "Runtime.executionContextCreated",
      "worker Runtime.executionContextCreated",
    );
    const context = (contextCreated.params as {
      context: {
        name: string;
        auxData: { isDefault: boolean; type: string };
      };
    }).context;
    assertEquals(context.auxData, { isDefault: true, type: "worker" });

    assertEquals(await tester.nextStdoutLine(), "Worker received: start");
    await tester.expectNotificationMatching(
      (msg) => msg.sessionId === sessionId && msg.method === "Debugger.paused",
      "worker Debugger.paused",
    );

    tester.send({ id: 9, sessionId, method: "Debugger.resume" });
    await tester.expectResponse(9);
    assertEquals(await tester.nextStdoutLine(), "after debugger");
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
    await Deno.remove(tempDir, { recursive: true });
  }
});

Deno.test("inspector_worker_step_over_creation_waits_for_debugger", async () => {
  const tempDir = await Deno.makeTempDir();
  const mainScript = `${tempDir}/main.js`;
  const workerScript = `${tempDir}/worker.js`;
  await Deno.writeTextFile(
    mainScript,
    `
const worker = new Worker(new URL("./worker.js", import.meta.url).href, {
  type: "module",
});

worker.postMessage("start");
setInterval(() => {}, 1000);
`,
  );
  await Deno.writeTextFile(
    workerScript,
    `
self.onmessage = (e) => {
  debugger;
  console.log("Worker received:", e.data);
  console.log("aa");
};
setInterval(() => {}, 1000);
`,
  );

  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect-brk=0", mainScript],
    { notificationFilter: ignoreScriptParsed, timeout: 5_000 },
  );

  try {
    await tester.assertStderrForInspectBrk();

    tester.sendMany([
      { id: 1, method: "Runtime.enable" },
      { id: 2, method: "Debugger.enable" },
      {
        id: 3,
        method: "Target.setAutoAttach",
        params: {
          autoAttach: true,
          waitForDebuggerOnStart: false,
          flatten: true,
        },
      },
      { id: 4, method: "Runtime.runIfWaitingForDebugger" },
    ]);

    await tester.expectResponse(1);
    await tester.expectResponse(2);
    await tester.expectResponse(3);
    await tester.expectResponse(4);
    await tester.expectNotification("Runtime.executionContextCreated");
    await tester.expectNotification("Debugger.paused");

    tester.send({ id: 5, method: "Debugger.stepOver" });
    await tester.expectResponse(5);
    await tester.expectNotification("Debugger.resumed");
    await tester.expectNotification("Debugger.paused");

    await new Promise((resolve) => setTimeout(resolve, 750));

    tester.send({ id: 6, method: "Debugger.resume" });
    await tester.expectResponse(6);
    await tester.expectNotification("Debugger.resumed");

    const attached = await tester.expectNotification("Target.attachedToTarget");
    const attachedParams = attached.params as Record<string, unknown>;
    assertEquals(attachedParams.waitingForDebugger, false);
    const sessionId = attachedParams.sessionId as string;
    assert(sessionId, "attachedToTarget should include sessionId");

    await new Promise((resolve) => setTimeout(resolve, 100));

    tester.sendMany([
      { id: 7, sessionId, method: "Page.waitForDebugger" },
      { id: 8, sessionId, method: "Runtime.enable" },
      { id: 9, sessionId, method: "Debugger.enable" },
      {
        id: 10,
        sessionId,
        method: "Runtime.runIfWaitingForDebugger",
      },
    ]);
    await tester.expectResponse(7);
    await tester.expectResponse(8);
    await tester.expectResponse(9);
    await tester.expectResponse(10);

    await tester.expectNotificationMatching(
      (msg) => msg.sessionId === sessionId && msg.method === "Debugger.paused",
      "worker Debugger.paused",
    );

    tester.send({ id: 11, sessionId, method: "Debugger.resume" });
    await tester.expectResponse(11);
    assertEquals(await tester.nextStdoutLine(), "Worker received: start");
    assertEquals(await tester.nextStdoutLine(), "aa");
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
    await Deno.remove(tempDir, { recursive: true });
  }
});

// Regression test for https://github.com/denoland/deno/issues/34291
// vscode-js-debug calls NodeWorker.enable before the user script runs, so
// any Worker constructor fires *after* NodeWorker.enable has been processed.
// Previously these later workers were not announced — NodeWorker.enable
// only walked existing workers, and the new-worker registration path only
// emitted Target.* events. Now NodeWorker.attachedToWorker fires for both.
Deno.test("inspector_node_worker_attached_after_enable", async () => {
  const script = `${testdataPath}/worker_main.js`;
  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect-brk=0", script],
    { notificationFilter: ignoreScriptParsed },
  );

  try {
    await tester.assertStderrForInspectBrk();

    tester.sendMany([
      { id: 1, method: "Runtime.enable" },
      { id: 2, method: "Debugger.enable" },
      {
        id: 3,
        method: "NodeWorker.enable",
        params: { waitForDebuggerOnStart: false },
      },
      { id: 4, method: "Runtime.runIfWaitingForDebugger" },
    ]);

    await tester.expectResponse(1);
    await tester.expectResponse(2);
    await tester.expectResponse(3);
    await tester.expectResponse(4);
    await tester.expectNotification("Runtime.executionContextCreated");
    await tester.expectNotification("Debugger.paused");

    tester.send({ id: 5, method: "Debugger.resume" });
    await tester.expectResponse(5);

    const attached = await tester.expectNotification(
      "NodeWorker.attachedToWorker",
    );
    const params = attached.params as Record<string, unknown>;
    assert(params.sessionId, "attachedToWorker should include sessionId");
    const workerInfo = params.workerInfo as Record<string, unknown>;
    assert(workerInfo, "attachedToWorker should include workerInfo");
    assertEquals(workerInfo.type, "node_worker");
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

Deno.test("inspector_node_worker_enable", async () => {
  const script = `${testdataPath}/worker_main.js`;
  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect-brk=0", script],
    { notificationFilter: ignoreScriptParsed },
  );

  try {
    await tester.assertStderrForInspectBrk();

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
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

Deno.test("inspector_noderuntime_waiting_for_debugger", async () => {
  // Verifies that NodeRuntime.enable emits NodeRuntime.waitingForDebugger
  // when --inspect-brk is used, and that Runtime.runIfWaitingForDebugger
  // unblocks execution and triggers Debugger.paused.
  const script = `${testdataPath}/inspector2.js`;
  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect-brk=0", script],
    { notificationFilter: ignoreScriptParsed },
  );

  try {
    await tester.assertStderrForInspectBrk();

    // Enable NodeRuntime first - should emit waitingForDebugger
    tester.send({ id: 1, method: "NodeRuntime.enable" });
    await tester.expectNotification("NodeRuntime.waitingForDebugger");

    // Now enable Runtime and Debugger
    tester.sendMany([
      { id: 2, method: "Runtime.enable" },
      { id: 3, method: "Debugger.enable" },
    ]);

    await tester.expectResponse(2);
    await tester.expectResponse(3);

    // Resume - should unblock and pause at first statement
    tester.send({ id: 4, method: "Runtime.runIfWaitingForDebugger" });
    await tester.expectResponse(4);
    await tester.expectNotification("Debugger.paused");

    // Resume execution
    tester.send({ id: 5, method: "Debugger.resume" });
    await tester.expectResponse(5);

    // Script should run to completion
    const scriptOutput = await tester.nextStdoutLine();
    assertEquals(scriptOutput, "hello from the script");
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

Deno.test("inspector_runtime_evaluate_does_not_crash", async () => {
  const tester = await InspectorTester.create(
    ["repl", "-A", "--inspect=0"],
    {
      notificationFilter: ignoreScriptParsed,
      env: { RUST_BACKTRACE: "1" },
    },
  );

  try {
    await tester.assertStderrForInspect();

    const banner = await tester.nextStdoutLine();
    assert(banner.startsWith("Deno"), `Expected Deno banner, got: ${banner}`);
    const exitMsg = await tester.nextStdoutLine();
    assertEquals(exitMsg, "exit using ctrl+d, ctrl+c, or close()");

    const sessionLine = await tester.nextStderrLine();
    assertEquals(sessionLine, "Debugger session started.");

    tester.sendMany([
      { id: 1, method: "Runtime.enable" },
      { id: 2, method: "Debugger.enable" },
    ]);

    await tester.expectResponse(1, { prefixMatch: '{"id":1,"result":{}}' });
    await tester.expectResponse(2, {
      prefixMatch: '{"id":2,"result":{"debuggerId":',
    });
    await tester.expectNotification("Runtime.executionContextCreated");

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

    const doneLine = await tester.nextStderrLine();
    assertEquals(doneLine, "done");

    await tester.stdin.close();
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

// Regression test for "Promise was collected" CDP errors when an external
// debugger sends `Runtime.evaluate({replMode: true})` to `deno repl --inspect`.
// V8 inspector wraps replMode evaluations in `(async () => EXPR)()` and tracks
// the result promise via a weak handle — without an immediate microtask drain
// after each event-loop poll, GC can collect the promise before its resolution
// microtask runs. `--gc-interval=100` forces a major GC every 100 allocations,
// which deterministically exposes the race.
Deno.test("inspector_repl_runtime_evaluate_replmode_under_gc", async () => {
  const tester = await InspectorTester.create(
    ["repl", "-A", "--inspect=0", "--v8-flags=--gc-interval=100"],
    {
      notificationFilter: ignoreScriptParsed,
      env: { RUST_BACKTRACE: "1" },
    },
  );

  try {
    await tester.assertStderrForInspect();
    await tester.nextStdoutLine(); // banner
    await tester.nextStdoutLine(); // exit hint
    await tester.nextStderrLine(); // "Debugger session started."

    tester.sendMany([
      { id: 1, method: "Runtime.enable" },
      { id: 2, method: "Debugger.enable" },
    ]);
    await tester.expectResponse(1, { prefixMatch: '{"id":1,"result":{}}' });
    await tester.expectResponse(2, {
      prefixMatch: '{"id":2,"result":{"debuggerId":',
    });
    await tester.expectNotification("Runtime.executionContextCreated");

    for (let i = 0; i < 50; i++) {
      const id = 100 + i;
      tester.send({
        id,
        method: "Runtime.evaluate",
        params: {
          expression: `new Array(2048).fill({}); ${i}`,
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
      const resp = await tester.expectResponse(id);
      assertEquals(
        resp.error,
        undefined,
        `iter ${i}: unexpected error ${JSON.stringify(resp.error)}`,
      );
    }

    await tester.stdin.close();
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

Deno.test("inspector_break_on_first_line_in_test", async () => {
  if (Deno.build.os === "windows") return;

  const script = `${testdataPath}/inspector_test.js`;
  const tester = await InspectorTester.create(
    ["test", "-A", "--inspect-brk=0", script],
    {
      notificationFilter: ignoreScriptParsed,
      env: { NO_COLOR: "1" },
    },
  );

  try {
    await tester.assertStderrForInspectBrk();

    tester.sendMany([
      { id: 1, method: "Runtime.enable" },
      { id: 2, method: "Debugger.enable" },
    ]);

    await tester.expectResponse(1, { prefixMatch: '{"id":1,"result":{}}' });
    await tester.expectResponse(2, {
      prefixMatch: '{"id":2,"result":{"debuggerId":',
    });
    await tester.expectNotification("Runtime.executionContextCreated");

    tester.send({ id: 3, method: "Runtime.runIfWaitingForDebugger" });
    await tester.expectResponse(3, { prefixMatch: '{"id":3,"result":{}}' });
    await tester.expectNotification("Debugger.paused");

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

    tester.send({ id: 5, method: "Debugger.resume" });
    await tester.expectResponse(5, { prefixMatch: '{"id":5,"result":{}}' });

    const line1 = await tester.nextStdoutLine();
    assert(
      line1.includes("running 1 test from"),
      `Expected test start, got: ${line1}`,
    );
    const line2 = await tester.nextStdoutLine();
    assert(line2.includes("basic test ... ok"), `Expected ok, got: ${line2}`);
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

// Regression test for https://github.com/denoland/deno/issues/19289.
// `deno test --inspect-brk` must wait for an attached debugger that opted into
// `NodeRuntime.notifyWhenWaitingForDisconnect` (as Chrome DevTools does)
// before exiting; otherwise an in-progress Performance recording is dropped
// the moment the test finishes.
async function assertTestWaitsForDebuggerDisconnect(args: string[]) {
  const tester = await InspectorTester.create(
    args,
    {
      notificationFilter: ignoreScriptParsed,
      env: { NO_COLOR: "1" },
    },
  );

  try {
    await tester.assertStderrForInspectBrk();

    tester.sendMany([
      { id: 1, method: "Runtime.enable" },
      { id: 2, method: "Debugger.enable" },
      {
        id: 3,
        method: "NodeRuntime.notifyWhenWaitingForDisconnect",
        params: { enabled: true },
      },
    ]);

    await tester.expectResponse(1);
    await tester.expectResponse(2, {
      prefixMatch: '{"id":2,"result":{"debuggerId":',
    });
    await tester.expectResponse(3);
    await tester.expectNotification("Runtime.executionContextCreated");

    tester.send({ id: 4, method: "Runtime.runIfWaitingForDebugger" });
    await tester.expectResponse(4);
    await tester.expectNotification("Debugger.paused");

    tester.send({ id: 5, method: "Debugger.resume" });
    await tester.expectResponse(5);

    // The test must run to completion. Before the fix the process would
    // race past the inspector wait and exit before the runtime emitted
    // `Runtime.executionContextDestroyed`, leaving the websocket dangling.
    const line1 = await tester.nextStdoutLine();
    assert(
      line1.includes("running 1 test from"),
      `Expected test start, got: ${line1}`,
    );
    const line2 = await tester.nextStdoutLine();
    assert(line2.includes("basic test ... ok"), `Expected ok, got: ${line2}`);

    // The runtime should now be parked waiting for us to disconnect. Wait
    // for the context-destroyed notification; proof that the wait kicked
    // in, since on the buggy path the process would exit instead.
    await tester.expectNotification("Runtime.executionContextDestroyed");

    // Closing the socket signals the debugger has disconnected; the runtime
    // should now exit cleanly with code 0.
    await tester.close();
    const status = await tester.waitForExit();
    assertEquals(status.code, 0);
  } finally {
    tester.kill();
    await tester.waitForExit();
  }
}

Deno.test("inspector_test_waits_for_debugger_disconnect", async () => {
  if (Deno.build.os === "windows") return;

  const script = `${testdataPath}/inspector_test.js`;
  await assertTestWaitsForDebuggerDisconnect([
    "test",
    "-A",
    "--inspect-brk=0",
    script,
  ]);
});

Deno.test("inspector_test_coverage_does_not_block_shutdown", async () => {
  const coverageDir = await Deno.makeTempDir();
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  let timedOut = false;
  try {
    const script = `${testdataPath}/inspector_test.js`;

    // `--coverage` uses a local blocking inspector session internally. It is
    // mutually exclusive with `--inspect-brk`, but it still exercises the
    // shutdown ordering that must stop coverage before waiting for inspector
    // sessions to disconnect.
    const child = new Deno.Command(Deno.execPath(), {
      args: ["test", "-A", `--coverage=${coverageDir}`, script],
      stdout: "piped",
      stderr: "piped",
      env: { NO_COLOR: "1" },
    }).spawn();

    timeoutId = setTimeout(() => {
      timedOut = true;
      try {
        child.kill("SIGKILL");
      } catch {
        // Process may have exited between the timeout firing and kill.
      }
    }, 10_000);

    const output = await child.output();
    const stdout = new TextDecoder().decode(output.stdout);
    const stderr = new TextDecoder().decode(output.stderr);
    assert(
      !timedOut,
      `deno test --coverage did not exit cleanly.\nstdout:\n${stdout}\nstderr:\n${stderr}`,
    );
    assertEquals(output.code, 0, `stdout:\n${stdout}\nstderr:\n${stderr}`);
    assertStringIncludes(stdout, "basic test ... ok");
    const coverageFiles = [];
    for await (const entry of Deno.readDir(coverageDir)) {
      if (entry.isFile) coverageFiles.push(entry.name);
    }
    assert(
      coverageFiles.length > 0,
      `Expected coverage output.\nstdout:\n${stdout}\nstderr:\n${stderr}`,
    );
  } finally {
    if (timeoutId !== undefined) {
      clearTimeout(timeoutId);
    }
    await Deno.remove(coverageDir, { recursive: true });
  }
});

Deno.test("inspector_with_ts_files", async () => {
  const script = `${testdataPath}/test.ts`;

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
      "-A",
      "--check",
      "--inspect-brk=0",
      script,
    ],
    { notificationFilter },
  );

  try {
    await tester.assertStderrForInspectBrk();

    let sessionLine = "";
    const sessionDeadline = Date.now() + 10000;
    while (Date.now() < sessionDeadline) {
      sessionLine = await tester.nextStderrLine();
      if (sessionLine === "Debugger session started.") break;
    }
    assertEquals(sessionLine, "Debugger session started.");

    tester.sendMany([
      { id: 1, method: "Runtime.enable" },
      { id: 2, method: "Debugger.enable" },
    ]);

    await tester.expectResponse(1, { prefixMatch: '{"id":1,"result":{}}' });
    await tester.expectNotification("Runtime.executionContextCreated");

    const scripts: { url: string; scriptId: string }[] = [];
    const deadline = Date.now() + 30000;
    let debuggerResponse: CDPMessage | null = null;

    while (scripts.length < 3 && Date.now() < deadline) {
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
        // No scriptParsed notification available
      }

      if (!debuggerResponse) {
        try {
          debuggerResponse = await tester.expectResponse(2, { timeout: 100 });
        } catch {
          // Not ready yet
        }
      }
    }

    if (!debuggerResponse) {
      debuggerResponse = await tester.expectResponse(2);
    }

    const testTs = scripts.find((s) => s.url.includes("test.ts"));
    const fooTs = scripts.find((s) => s.url.includes("foo.ts"));
    const barJs = scripts.find((s) => s.url.includes("bar.js"));

    assert(testTs, "Should have test.ts");
    assert(fooTs, "Should have foo.ts");
    assert(barJs, "Should have bar.js");

    tester.send({ id: 3, method: "Runtime.runIfWaitingForDebugger" });
    await tester.expectResponse(3);
    await tester.expectNotification("Debugger.paused");

    tester.sendMany([
      {
        id: 4,
        method: "Debugger.getScriptSource",
        params: { scriptId: testTs!.scriptId },
      },
      {
        id: 5,
        method: "Debugger.getScriptSource",
        params: { scriptId: fooTs!.scriptId },
      },
      {
        id: 6,
        method: "Debugger.getScriptSource",
        params: { scriptId: barJs!.scriptId },
      },
    ]);

    const source1 = await tester.expectResponse(4);
    const result1 = source1.result as Record<string, string>;
    assert(
      result1.scriptSource.includes('import { foo } from "./foo.ts"'),
      "test.ts should have foo import",
    );

    const source2 = await tester.expectResponse(5);
    const result2 = source2.result as Record<string, string>;
    assert(
      result2.scriptSource.includes("class Foo"),
      "foo.ts should have class Foo",
    );

    const source3 = await tester.expectResponse(6);
    const result3 = source3.result as Record<string, string>;
    assert(
      result3.scriptSource.includes("export function bar"),
      "bar.js should have bar function",
    );

    tester.send({ id: 7, method: "Debugger.resume" });
    await tester.expectResponse(7);

    assertEquals(await tester.nextStdoutLine(), "hello");
    assertEquals(await tester.nextStdoutLine(), "world");
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

Deno.test("inspector_wait", async () => {
  const script = `${testdataPath}/inspect_wait.js`;
  const tempDir = await Deno.makeTempDir();
  const helloPath = `${tempDir}/hello.txt`;

  const command = new Deno.Command(Deno.execPath(), {
    args: ["run", "-A", "--inspect-wait=0", script],
    stdin: "piped",
    stdout: "piped",
    stderr: "piped",
    cwd: tempDir,
  });

  const child = command.spawn();
  const stderrReader = child.stderr.pipeThrough(new TextDecoderStream())
    .getReader();
  const stdoutReader = child.stdout.pipeThrough(new TextDecoderStream())
    .getReader();

  try {
    await new Promise((r) => setTimeout(r, 300));

    let fileExists = false;
    try {
      await Deno.stat(helloPath);
      fileExists = true;
    } catch {
      // Expected - file shouldn't exist yet
    }
    assert(!fileExists, "File should not exist before debugger connects");

    const wsUrl = await extractWsUrl(stderrReader);
    const socket = new WebSocket(wsUrl);
    await new Promise<void>((resolve, reject) => {
      socket.onopen = () => resolve();
      socket.onerror = (e) => reject(e);
    });

    let msgId = 1;
    const send = (msg: Record<string, unknown>) => {
      socket.send(JSON.stringify(msg));
    };
    socket.onmessage = () => {};

    send({ id: msgId++, method: "Runtime.enable" });
    send({ id: msgId++, method: "Debugger.enable" });

    await new Promise((r) => setTimeout(r, 500));

    send({ id: msgId++, method: "Runtime.runIfWaitingForDebugger" });

    let stderrContent = "";
    const deadline = Date.now() + 5000;
    while (Date.now() < deadline) {
      const { value, done } = await stderrReader.read();
      if (done || !value) break;
      stderrContent += value;
      if (stderrContent.includes("did run")) break;
    }

    assert(
      stderrContent.includes("did run"),
      `Expected 'did run' in stderr: ${stderrContent}`,
    );

    try {
      await Deno.stat(helloPath);
      fileExists = true;
    } catch {
      fileExists = false;
    }
    assert(fileExists, "File should exist after script runs");

    socket.close();
  } finally {
    await child.stdin.close();
    await stderrReader.cancel();
    await stdoutReader.cancel();
    child.kill();
    await child.status;
    try {
      await Deno.remove(tempDir, { recursive: true });
    } catch {
      // Ignore cleanup errors
    }
  }
});

Deno.test("inspector_node_wait_for_debugger_no_pause", async () => {
  // Regression test: node:inspector waitForDebugger() must block until a
  // session sends Runtime.runIfWaitingForDebugger and then resume WITHOUT
  // scheduling a pause on the next statement. It used the --inspect-brk
  // primitive, so clients received an unexpected Debugger.paused right
  // after attaching (VS Code stopped inside js-debug's bootloader).
  const preload = `${testdataPath}/wait_for_debugger_preload.cjs`;
  const script = `${testdataPath}/wait_for_debugger_program.js`;
  const tempDir = await Deno.makeTempDir();
  const helloPath = `${tempDir}/hello.txt`;

  const command = new Deno.Command(Deno.execPath(), {
    args: ["run", "-A", "--require", preload, script],
    stdin: "piped",
    stdout: "piped",
    stderr: "piped",
    cwd: tempDir,
  });

  const child = command.spawn();
  const stderrReader = child.stderr.pipeThrough(new TextDecoderStream())
    .getReader();
  const stdoutReader = child.stdout.pipeThrough(new TextDecoderStream())
    .getReader();

  try {
    const wsUrl = await extractWsUrl(stderrReader);
    const socket = new WebSocket(wsUrl);
    await new Promise<void>((resolve, reject) => {
      socket.onopen = () => resolve();
      socket.onerror = (e) => reject(e);
    });

    let msgId = 1;
    const send = (msg: Record<string, unknown>) => {
      socket.send(JSON.stringify(msg));
    };
    let paused = false;
    socket.onmessage = (e) => {
      const message = JSON.parse(e.data);
      if (message.method === "Debugger.paused") {
        paused = true;
        send({ id: msgId++, method: "Debugger.resume" });
      }
    };

    send({ id: msgId++, method: "Runtime.enable" });
    send({ id: msgId++, method: "Debugger.enable" });

    // Give the child a moment: the program must NOT run before the
    // session sends Runtime.runIfWaitingForDebugger.
    await new Promise((r) => setTimeout(r, 500));

    let fileExists = false;
    try {
      await Deno.stat(helloPath);
      fileExists = true;
    } catch {
      // Expected - file shouldn't exist yet
    }
    assert(
      !fileExists,
      "waitForDebugger() did not block until the debugger resumed",
    );

    send({ id: msgId++, method: "Runtime.runIfWaitingForDebugger" });

    // If a pause was (incorrectly) scheduled, the program stops before its
    // first statement; the Debugger.paused handler above resumes it, so
    // "did run" still arrives and `paused` records the regression.
    let stderrContent = "";
    const deadline = Date.now() + 5000;
    while (Date.now() < deadline) {
      const { value, done } = await stderrReader.read();
      if (done || !value) break;
      stderrContent += value;
      if (stderrContent.includes("did run")) break;
    }
    assert(
      stderrContent.includes("did run"),
      `Expected 'did run' in stderr: ${stderrContent}`,
    );

    assert(!paused, "received an unexpected Debugger.paused after resuming");

    socket.close();
  } finally {
    await child.stdin.close();
    await stderrReader.cancel();
    await stdoutReader.cancel();
    child.kill();
    await child.status;
    try {
      await Deno.remove(tempDir, { recursive: true });
    } catch {
      // Ignore cleanup errors
    }
  }
});

Deno.test("inspector_console_api_late_open", async () => {
  // Regression test: when the inspector is activated at runtime via
  // node:inspector open() (instead of a CLI --inspect* flag), sessions must
  // still receive Runtime.consoleAPICalled for console calls. The console
  // was only bridged to the V8 inspector console when an --inspect* flag
  // was present at bootstrap, so late-opened inspectors saw no console
  // output.
  const preload = `${testdataPath}/console_api_late_open_preload.cjs`;
  const script = `${testdataPath}/console_api_late_open_logger.js`;

  const command = new Deno.Command(Deno.execPath(), {
    args: ["run", "-A", "--require", preload, script],
    stdin: "piped",
    stdout: "piped",
    stderr: "piped",
  });

  const child = command.spawn();
  const stderrReader = child.stderr.pipeThrough(new TextDecoderStream())
    .getReader();
  const stdoutReader = child.stdout.pipeThrough(new TextDecoderStream())
    .getReader();

  try {
    const wsUrl = await extractWsUrl(stderrReader);
    const socket = new WebSocket(wsUrl);
    await new Promise<void>((resolve, reject) => {
      socket.onopen = () => resolve();
      socket.onerror = (e) => reject(e);
    });

    const consoleApiCalled = new Promise<void>((resolve) => {
      socket.onmessage = (e) => {
        const message = JSON.parse(e.data);
        if (
          message.method === "Runtime.consoleAPICalled" &&
          message.params.args[0]?.value === "tick"
        ) {
          resolve();
        }
      };
    });

    socket.send(JSON.stringify({ id: 1, method: "Runtime.enable" }));

    await consoleApiCalled;

    socket.close();
  } finally {
    await child.stdin.close();
    await stderrReader.cancel();
    await stdoutReader.cancel();
    child.kill();
    await child.status;
  }
});

Deno.test("inspector_node_runtime_api_url", async () => {
  const script = `${testdataPath}/node/url.js`;

  const command = new Deno.Command(Deno.execPath(), {
    args: ["run", "-A", "--inspect=0", script],
    stdout: "piped",
    stderr: "piped",
  });

  const output = await command.output();
  const stderr = new TextDecoder().decode(output.stderr);
  const stdout = new TextDecoder().decode(output.stdout);

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
});

// Regression test for https://github.com/denoland/deno/issues/30176.
// `console.group(label)` must emit a paired `log` event so Chrome DevTools
// renders the label inside the group container; otherwise the group appears
// empty and the visible nesting drifts out of alignment with the CLI.
Deno.test("inspector_console_group_emits_label_log", async () => {
  const script = `${testdataPath}/inspector_group.js`;
  // `--inspect-wait` keeps the script paused until the inspector calls
  // `Runtime.runIfWaitingForDebugger`. Avoid `--inspect-brk` here: that mode
  // also pauses on the first line and would require an extra
  // `Debugger.resume` round-trip, which races with this test's expectations.
  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect-wait=0", script],
    { notificationFilter: ignoreScriptParsed },
  );

  try {
    tester.sendMany([
      { id: 1, method: "Runtime.enable" },
      { id: 2, method: "Debugger.enable" },
    ]);
    await tester.expectResponse(1);
    await tester.expectResponse(2);
    await tester.expectNotification("Runtime.executionContextCreated");

    tester.send({ id: 3, method: "Runtime.runIfWaitingForDebugger" });
    await tester.expectResponse(3);

    // Sentinel "done" log marks the end of the script-emitted console events.
    const events: Array<{ type: string; arg: string | undefined }> = [];
    while (true) {
      const msg = await tester.expectNotification("Runtime.consoleAPICalled");
      // deno-lint-ignore no-explicit-any
      const params = msg.params as any;
      const arg = params.args?.[0]?.value;
      events.push({ type: params.type, arg });
      if (params.type === "log" && arg === "done") break;
    }

    assertEquals(events, [
      { type: "startGroup", arg: "test1" },
      { type: "log", arg: "test1" },
      { type: "startGroup", arg: "test2" },
      { type: "log", arg: "test2" },
      { type: "endGroup", arg: "console.groupEnd" },
      { type: "endGroup", arg: "console.groupEnd" },
      { type: "log", arg: "done" },
    ]);
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

// Regression test for https://github.com/denoland/deno/issues/18513
// Piping a file stream to a WritableStream internally releases the writer,
// rejecting its ready/closed promises. Those rejections are handled, so a
// debugger with "pause on uncaught exceptions" enabled must not break on them.
Deno.test("inspector_no_pause_on_handled_stream_rejection", async () => {
  const script = `${testdataPath}/pipe_file_to_writable.js`;
  const tester = await InspectorTester.create(
    ["run", "-A", "--inspect-brk=0", script],
    { notificationFilter: ignoreScriptParsed },
  );

  try {
    await tester.assertStderrForInspectBrk();

    tester.sendMany([
      { id: 1, method: "Runtime.enable" },
      { id: 2, method: "Debugger.enable" },
      {
        id: 3,
        method: "Debugger.setPauseOnExceptions",
        params: { state: "uncaught" },
      },
    ]);

    await tester.expectResponse(1);
    await tester.expectResponse(2);
    await tester.expectResponse(3);
    await tester.expectNotification("Runtime.executionContextCreated");

    tester.send({ id: 4, method: "Runtime.runIfWaitingForDebugger" });
    await tester.expectResponse(4);

    // First pause is the --inspect-brk break on the first line.
    const firstPause = await tester.expectNotification("Debugger.paused");
    assert(
      // deno-lint-ignore no-explicit-any
      (firstPause.params as any)?.reason !== "exception" &&
        // deno-lint-ignore no-explicit-any
        (firstPause.params as any)?.reason !== "promiseRejection",
      "first pause should be the inspect-brk break, not an exception",
    );

    tester.send({ id: 5, method: "Debugger.resume" });
    await tester.expectResponse(5);

    // Releasing the writer and reader during pipeTo rejects their internal
    // ready/closed promises, but those rejections are handled. With
    // "pause on uncaught exceptions" enabled the debugger must not break on
    // them. If it regresses, a Debugger.paused (reason "promiseRejection")
    // arrives here instead of the script running to completion.
    let unexpectedPause: CDPMessage | undefined;
    try {
      unexpectedPause = await tester.expectNotification("Debugger.paused", {
        timeout: 5_000,
      });
    } catch {
      // No second pause within the window — this is the expected outcome.
    }
    if (unexpectedPause) {
      // deno-lint-ignore no-explicit-any
      const params = unexpectedPause.params as any;
      throw new Error(
        `debugger paused unexpectedly on a handled rejection: reason=${params?.reason} ${
          params?.data?.description ?? ""
        }`,
      );
    }

    const scriptOutput = await tester.nextStdoutLine();
    assertEquals(scriptOutput, "done");
  } finally {
    await tester.close();
    tester.kill();
    await tester.waitForExit();
  }
});

Deno.test({
  name: "inspector_starts_on_sigusr1",
  ignore: Deno.build.os === "windows",
  permissions: { run: true, read: true, net: true },
  async fn() {
    const script = testdataPath + "sigusr1.js";
    const child = new Deno.Command(Deno.execPath(), {
      args: ["run", script],
      stdout: "piped",
      stderr: "piped",
    }).spawn();
    const stdoutReader = child.stdout
      .pipeThrough(new TextDecoderStream())
      .getReader();
    const stderrReader = child.stderr
      .pipeThrough(new TextDecoderStream())
      .getReader();
    try {
      // Wait until the SIGUSR1 listener's grace period has passed.
      let stdoutBuffer = "";
      while (!stdoutBuffer.includes("ready")) {
        const { value, done } = await stdoutReader.read();
        if (done) throw new Error("child exited before printing ready");
        stdoutBuffer += value;
      }

      child.kill("SIGUSR1");
      const wsUrl = await extractWsUrl(stderrReader);
      assertStringIncludes(wsUrl, "ws://127.0.0.1:9229/");

      // The /json endpoint should list the main module as a target.
      const response = await fetch("http://127.0.0.1:9229/json");
      const targets = await response.json();
      assertEquals(targets.length, 1);
      assertStringIncludes(targets[0].url, "sigusr1.js");

      // A second SIGUSR1 must be a no-op, not crash the process.
      child.kill("SIGUSR1");
      const secondResponse = await fetch("http://127.0.0.1:9229/json");
      assertEquals((await secondResponse.json()).length, 1);
    } finally {
      child.kill("SIGKILL");
      await child.status;
      await stdoutReader.cancel().catch(() => {});
      await stderrReader.cancel().catch(() => {});
    }
  },
});
