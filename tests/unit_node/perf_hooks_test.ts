// Copyright 2018-2026 the Deno authors. MIT license.
import {
  monitorEventLoopDelay,
  performance,
  PerformanceEntry,
  PerformanceObserver,
} from "node:perf_hooks";
import http from "node:http";
import type { AddressInfo } from "node:net";
import { assert, assertEquals, assertThrows } from "@std/assert";

// Basic performance API tests removed - covered by Node compat tests:
// - parallel/test-performance-global.js
// - parallel/test-performanceobserver-gc.js

Deno.test({
  name: "[perf_hooks] performance.timeOrigin",
  fn() {
    assertEquals(typeof performance.timeOrigin, "number");
    assertThrows(() => {
      // @ts-expect-error: Cannot assign to 'timeOrigin' because it is a read-only property
      performance.timeOrigin = 1;
    });
  },
});

Deno.test("[perf_hooks]: eventLoopUtilization", () => {
  const obj = performance.eventLoopUtilization();
  assertEquals(typeof obj.idle, "number");
  assertEquals(typeof obj.active, "number");
  assertEquals(typeof obj.utilization, "number");
});

Deno.test("[perf_hooks]: monitorEventLoopDelay", async () => {
  const e = monitorEventLoopDelay();
  assertEquals(e.count, 0);
  e.enable();

  await new Promise((resolve) => setTimeout(resolve, 100));

  assert(e.min > 0);
  assert(e.minBigInt > 0n);
  assert(e.count > 0);

  e.disable();
});

Deno.test("[perf_hooks]: markResourceTiming", () => {
  assert(typeof performance.markResourceTiming === "function");
});

Deno.test("[perf_hooks]: PerformanceObserver.supportedEntryTypes", () => {
  const supported = PerformanceObserver.supportedEntryTypes;
  assert(Array.isArray(supported));
  assert(supported.includes("mark"));
  assert(supported.includes("measure"));
});

Deno.test("[perf_hooks]: PerformanceObserver observes marks", async () => {
  const entries: PerformanceEntry[] = [];
  const observer = new PerformanceObserver((list) => {
    entries.push(...list.getEntries());
  });
  observer.observe({ entryTypes: ["mark"] });

  performance.mark("test-mark-1");
  performance.mark("test-mark-2");

  // Wait for microtask queue to flush
  await new Promise((resolve) => setTimeout(resolve, 10));

  assertEquals(entries.length, 2);
  assertEquals(entries[0].name, "test-mark-1");
  assertEquals(entries[1].name, "test-mark-2");
  assertEquals(entries[0].entryType, "mark");

  observer.disconnect();
  performance.clearMarks();
});

Deno.test("[perf_hooks]: PerformanceObserver observes measures", async () => {
  const entries: PerformanceEntry[] = [];
  const observer = new PerformanceObserver((list) => {
    entries.push(...list.getEntries());
  });
  observer.observe({ entryTypes: ["measure"] });

  performance.mark("start");
  performance.measure("test-measure", "start");

  await new Promise((resolve) => setTimeout(resolve, 10));

  assertEquals(entries.length, 1);
  assertEquals(entries[0].name, "test-measure");
  assertEquals(entries[0].entryType, "measure");

  observer.disconnect();
  performance.clearMarks();
  performance.clearMeasures();
});

Deno.test("[perf_hooks]: PerformanceObserver disconnect stops observation", async () => {
  const entries: PerformanceEntry[] = [];
  const observer = new PerformanceObserver((list) => {
    entries.push(...list.getEntries());
  });
  observer.observe({ entryTypes: ["mark"] });

  performance.mark("before-disconnect");
  await new Promise((resolve) => setTimeout(resolve, 10));

  observer.disconnect();

  performance.mark("after-disconnect");
  await new Promise((resolve) => setTimeout(resolve, 10));

  assertEquals(entries.length, 1);
  assertEquals(entries[0].name, "before-disconnect");

  performance.clearMarks();
});

Deno.test("[perf_hooks]: PerformanceObserver takeRecords", () => {
  const observer = new PerformanceObserver(() => {});
  observer.observe({ entryTypes: ["mark"] });

  performance.mark("take-records-test");

  const records = observer.takeRecords();
  assertEquals(records.length, 1);
  assertEquals(records[0].name, "take-records-test");

  // After takeRecords, buffer should be empty
  const secondRecords = observer.takeRecords();
  assertEquals(secondRecords.length, 0);

  observer.disconnect();
  performance.clearMarks();
});

Deno.test("[perf_hooks]: PerformanceObserver observes node:http server entries", async () => {
  const server = http.createServer((_req, res) => {
    res.end("ok");
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));

  let timer: ReturnType<typeof setTimeout> | undefined;
  let observer: PerformanceObserver | undefined;
  const entryPromise = new Promise<PerformanceEntry>((resolve, reject) => {
    timer = setTimeout(
      () => reject(new Error("Timed out waiting for HTTP performance entry")),
      1000,
    );
    observer = new PerformanceObserver((list) => {
      const entry = list.getEntries().find((entry) =>
        entry.entryType === "http"
      );
      if (entry) {
        clearTimeout(timer);
        resolve(entry);
      }
    });
    observer.observe({ entryTypes: ["http"] });
  });

  try {
    const address = server.address();
    if (!address || typeof address !== "object") {
      throw new Error("Server did not listen on a TCP address");
    }
    const response = await fetch(`http://127.0.0.1:${address.port}/observed`);
    assertEquals(await response.text(), "ok");

    const entry = await entryPromise;
    assertEquals(entry.name, "HttpRequest");
    assertEquals(entry.entryType, "http");
    assert(entry.duration >= 0);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
    observer?.disconnect();
    await new Promise<void>((resolve, reject) =>
      server.close((err) => err ? reject(err) : resolve())
    );
  }
});

// Waits for the first `http` entry named `name` and returns its `detail`.
function observeHttpEntry(name: string) {
  const { promise, resolve, reject } = Promise.withResolvers<
    // deno-lint-ignore no-explicit-any
    any
  >();
  const timer = setTimeout(
    () => reject(new Error(`Timed out waiting for ${name} entry`)),
    5000,
  );
  const observer = new PerformanceObserver((list) => {
    for (const entry of list.getEntries()) {
      if (entry.entryType === "http" && entry.name === name) {
        clearTimeout(timer);
        // deno-lint-ignore no-explicit-any
        resolve((entry as any).detail);
      }
    }
  });
  observer.observe({ entryTypes: ["http"] });
  return {
    detail: promise,
    dispose() {
      clearTimeout(timer);
      observer.disconnect();
    },
  };
}

// The reported URL must include the port when it is non-default, rather than
// falling back to the bare hostname.
// Refs: https://github.com/nodejs/node/issues/59625
Deno.test("[perf_hooks]: HttpClient url reports a non-default port", async () => {
  const server = http.createServer((_req, res) => res.end("ok"));
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = (server.address() as AddressInfo).port;

  const observed = observeHttpEntry("HttpClient");
  try {
    await new Promise<void>((resolve, reject) => {
      http.request({
        hostname: "127.0.0.1",
        port,
        path: "/observed",
        // No explicit Host header - the URL has to come from the request
        // authority, not from `req.host`, which carries no port.
        setHost: false,
      }, (res) => {
        res.resume();
        res.on("end", resolve);
      }).on("error", reject).end();
    });

    const detail = await observed.detail;
    assertEquals(detail.req.url, `http://127.0.0.1:${port}/observed`);
  } finally {
    observed.dispose();
    await new Promise<void>((resolve, reject) =>
      server.close((err) => err ? reject(err) : resolve())
    );
  }
});

// When a request path has been rewritten to absolute-form for proxying, the
// entry must report it as-is instead of appending it to the authority again.
// Refs: https://github.com/nodejs/node/issues/59625
Deno.test("[perf_hooks]: HttpClient url is not duplicated when proxied", async () => {
  // Short-circuiting proxy: it never forwards, so the target need not exist.
  const proxy = http.createServer((_req, res) => res.end("via-proxy"));
  await new Promise<void>((resolve) => proxy.listen(0, "127.0.0.1", resolve));
  const proxyPort = (proxy.address() as AddressInfo).port;

  const observed = observeHttpEntry("HttpClient");
  try {
    await new Promise<void>((resolve, reject) => {
      http.request({
        hostname: "127.0.0.1",
        port: 8080,
        path: "/foo",
        agent: new http.Agent({
          proxyEnv: { HTTP_PROXY: `http://127.0.0.1:${proxyPort}` },
          // deno-lint-ignore no-explicit-any
        } as any),
      }, (res) => {
        res.resume();
        res.on("end", resolve);
      }).on("error", reject).end();
    });

    const detail = await observed.detail;
    assertEquals(detail.req.url, "http://127.0.0.1:8080/foo");
  } finally {
    observed.dispose();
    await new Promise<void>((resolve, reject) =>
      proxy.close((err) => err ? reject(err) : resolve())
    );
  }
});

Deno.test("[perf_hooks]: node:http server entries are not retroactive", async () => {
  let finishResponse: (() => void) | undefined;
  let resolveRequestStarted = () => {};
  const requestStarted = new Promise<void>((resolve) => {
    resolveRequestStarted = resolve;
  });
  const server = http.createServer((_req, res) => {
    finishResponse = () => res.end("ok");
    resolveRequestStarted();
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));

  const entries: PerformanceEntry[] = [];
  const observer = new PerformanceObserver((list) => {
    entries.push(...list.getEntries());
  });

  try {
    const address = server.address();
    if (!address || typeof address !== "object") {
      throw new Error("Server did not listen on a TCP address");
    }
    const responsePromise = fetch(`http://127.0.0.1:${address.port}/late`);
    await requestStarted;

    observer.observe({ entryTypes: ["http"] });
    finishResponse?.();
    const response = await responsePromise;
    assertEquals(await response.text(), "ok");
    await new Promise((resolve) => setTimeout(resolve, 50));

    assertEquals(entries.length, 0);
  } finally {
    observer.disconnect();
    await new Promise<void>((resolve, reject) =>
      server.close((err) => err ? reject(err) : resolve())
    );
  }
});
