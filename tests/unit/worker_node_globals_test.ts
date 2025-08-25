// Copyright 2018-2025 the Deno authors. MIT license.

import { assert, assertEquals } from "./test_util.ts";

Deno.test("worker node globals", async function () {
  const script = `
    try {
      // Test all globals are available and have correct types
      const globalTypes = {
        global: typeof global,
        Buffer: typeof Buffer,
        process: typeof process,
        setImmediate: typeof setImmediate,
        clearImmediate: typeof clearImmediate,
      };
      
      // Test global reference correctness
      const globalRef = global === globalThis;
      
      // Test Buffer functionality (the specific failing case)
      const buffer = Buffer.from("Hello, World!", "utf8");
      
      self.postMessage({
        success: true,
        types: globalTypes,
        globalRef: globalRef,
        bufferLength: buffer.length,
        bufferFirstByte: buffer[0],
      });
    } catch (error) {
      self.postMessage({ 
        success: false, 
        error: error.message 
      });
    }
  `;

  const blob = new Blob([script], { type: "application/javascript" });
  const workerUrl = URL.createObjectURL(blob);

  try {
    const worker = new Worker(workerUrl, { type: "module" });
    const { promise, resolve } = Promise.withResolvers<{
      success: boolean;
      types?: {
        global: string;
        Buffer: string;
        process: string;
        setImmediate: string;
        clearImmediate: string;
      };
      globalRef?: boolean;
      bufferLength?: number;
      bufferFirstByte?: number;
      error?: string;
    }>();

    worker.onmessage = (e) => {
      worker.terminate();
      resolve(e.data);
    };

    worker.onerror = (e) => {
      worker.terminate();
      throw new Error(`Worker error: ${e.message}`);
    };

    const result = await promise;

    // Verify test succeeded
    assertEquals(result.success, true);
    assert(!result.error, `Node globals test failed: ${result.error}`);

    // Verify all globals have correct types
    assertEquals(result.types.global, "object");
    assertEquals(result.types.Buffer, "function");
    assertEquals(result.types.process, "object");
    assertEquals(result.types.setImmediate, "function");
    assertEquals(result.types.clearImmediate, "function");

    // Verify global reference correctness
    assertEquals(result.globalRef, true);

    // Verify Buffer functionality
    assertEquals(result.bufferLength, 13);
    assertEquals(result.bufferFirstByte, 72); // 'H' in UTF-8
  } finally {
    URL.revokeObjectURL(workerUrl);
  }
});
