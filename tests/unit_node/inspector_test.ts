// Copyright 2018-2025 the Deno authors. MIT license.
import inspector, { Session } from "node:inspector";
import inspectorPromises, {
  Session as SessionPromise,
} from "node:inspector/promises";
import { assertEquals, assertThrows } from "@std/assert";

Deno.test("[node/inspector] - importing inspector works", () => {
  assertEquals(typeof inspector.open, "function");
});

Deno.test("[node/inspector] - Session constructor should not throw", () => {
  new Session();
});

Deno.test("[node/inspector/promises] - importing inspector works", () => {
  assertEquals(typeof inspectorPromises.open, "function");
});

Deno.test("[node/inspector/promises] - Session constructor should not throw", () => {
  new SessionPromise();
});

// Regression test for: https://github.com/denoland/deno/issues/31020
Deno.test({
  name: "[node/inspector] - deeply nested session.post() calls",
  fn: async () => {
    const session = new Session();
    session.connect();

    const results: number[] = [];

    await new Promise<void>((resolve) => {
      session.post("Profiler.enable", () => {
        results.push(1);
        session.post("Profiler.start", () => {
          results.push(2);
          session.post("Profiler.stop", () => {
            results.push(3);
            session.post("Profiler.disable", () => {
              results.push(4);
              session.disconnect();
              resolve();
            });
          });
        });
      });
    });

    assertEquals(results, [1, 2, 3, 4]);
  },
});

Deno.test({
  name:
    "[node/inspector] - multiple session.post() calls in same callback are queued",
  fn: async () => {
    const session = new Session();
    session.connect();

    const results: string[] = [];

    await new Promise<void>((resolve) => {
      session.post("Profiler.enable", () => {
        results.push("enable-callback-start");

        // Make multiple session.post() calls in the same callback
        // These should be queued and processed in order
        session.post("Profiler.start", () => {
          results.push("start-callback");
        });

        session.post("Profiler.stop", () => {
          results.push("stop-callback");
        });

        session.post("Profiler.disable", () => {
          results.push("disable-callback");
          session.disconnect();
          resolve();
        });

        results.push("enable-callback-end");
      });
    });

    // Verify the outer callback completes first, then queued messages are processed
    assertEquals(results, [
      "enable-callback-start",
      "enable-callback-end",
      "start-callback",
      "stop-callback",
      "disable-callback",
    ]);
  },
});

Deno.test("[node/inspector] - url() requires sys permission", {
  permissions: { sys: false },
}, () => {
  assertThrows(() => inspector.url(), Deno.errors.NotCapable);
});

Deno.test("[node/inspector] - url() returns undefined when no --inspect flag", {
  permissions: { sys: true },
}, () => {
  const url = inspector.url();
  assertEquals(url, undefined);
});
