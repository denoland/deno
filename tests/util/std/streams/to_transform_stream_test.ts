// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertRejects } from "../assert/mod.ts";
import { toTransformStream } from "./to_transform_stream.ts";

Deno.test({
  name: "[streams] toTransformStream()",
  async fn() {
    const readable = ReadableStream.from([0, 1, 2])
      .pipeThrough(toTransformStream(async function* (src) {
        for await (const i of src) {
          yield i * 100;
        }
      }));

    const res = await Array.fromAsync(readable);
    assertEquals(res, [0, 100, 200]);
  },
});

Deno.test({
  name: "[streams] toTransformStream() Pass iterable instead of asyncIterable",
  async fn() {
    const readable = ReadableStream.from([0, 1, 2])
      .pipeThrough(toTransformStream(function* (_src) {
        yield 0;
        yield 100;
        yield 200;
      }));

    const res = await Array.fromAsync(readable);
    assertEquals(res, [0, 100, 200]);
  },
});

Deno.test({
  name: "[streams] toTransformStream() Propagate the error from readable 1",
  async fn(t) {
    // When data is pipelined in the order of readable1 → generator → readable2,
    // Propagate the error that occurred in readable1 to generator and readable2.
    const expectedError = new Error("Error from readable1");
    await t.step({
      name: "to readable 2",
      async fn() {
        // Propagate the error that occurred in readable1 to readable2.
        let actualError = null;

        const readable1 = new ReadableStream({
          start(controller) {
            controller.error(expectedError); // error from readable1
          },
        });
        const readable2 = readable1.pipeThrough(
          toTransformStream(async function* (src) {
            for await (const i of src) {
              yield i;
            }
          }),
        );

        try {
          await readable2.getReader().read();
        } catch (error) {
          actualError = error; // catch error in readable2
        }

        assertEquals(actualError, expectedError);
      },
    });
    await t.step({
      name: "to generator",
      async fn() {
        // Propagate the error that occurred in readable1 to generator.
        let actualError = null;

        const readable1 = new ReadableStream({
          start(controller) {
            controller.error(expectedError); // error from readable1
          },
        });
        const readable2 = readable1.pipeThrough(
          toTransformStream(async function* (src) {
            try {
              await src.getReader().read();
            } catch (error) {
              actualError = error; // catch error in generator
            }
            yield 0;
          }),
        );

        await readable2.getReader().read();
        assertEquals(actualError, expectedError);
      },
    });
  },
});

Deno.test({
  name: "[streams] toTransformStream() Propagate the error from generator",
  async fn(t) {
    // When data is pipelined in the order of readable1 → generator → readable2,
    // Propagate the error that occurred in generator to readable2 and readable1.
    const expectedError = new Error("Error from generator");
    let actualError1: unknown = null;
    let actualError2: unknown = null;

    const readable1 = new ReadableStream({
      cancel(reason) {
        actualError1 = reason; // catch error in readable1
      },
    });
    const readable2 = readable1.pipeThrough(
      // deno-lint-ignore require-yield
      toTransformStream(function* () {
        throw expectedError; // error from generator
      }),
    );

    try {
      await readable2.getReader().read();
    } catch (error) {
      actualError2 = error; // catch error in readable2
    }

    await t.step({
      name: "to readable 1",
      fn() {
        assertEquals(actualError1, expectedError);
      },
    });
    await t.step({
      name: "to readable 2",
      fn() {
        assertEquals(actualError2, expectedError);
      },
    });
  },
});

Deno.test({
  name: "[streams] toTransformStream() Propagate cancellation from readable 2",
  async fn(t) {
    // When data is pipelined in the order of readable1 → generator → readable2,
    // Propagate the cancellation that occurred in readable2 to readable1 and generator.
    const expectedError = new Error("Error from readable2");
    await t.step({
      name: "to readable 1",
      async fn() {
        let actualError = null;

        const readable1 = new ReadableStream({
          cancel(reason) {
            actualError = reason; // catch error in readable1
          },
        });
        const readable2 = readable1.pipeThrough(
          toTransformStream(function* () {
            yield 0;
          }),
        );

        await readable2.cancel(expectedError); // cancellation from readable2
        assertEquals(actualError, expectedError);
      },
    });
    await t.step({
      name: "to readable 2",
      async fn() {
        let actualError = null;

        const readable1 = new ReadableStream();
        const readable2 = readable1.pipeThrough(
          toTransformStream(function* () {
            try {
              yield 0;
            } catch (error) {
              actualError = error; // catch error in generator
            }
          }),
        );

        const reader = readable2.getReader();
        await reader.read();
        await reader.cancel(expectedError); // cancellation from readable2
        assertEquals(actualError, expectedError);
      },
    });
  },
});

Deno.test({
  name:
    "[streams] toTransformStream() Cancel streams with the correct error message",
  async fn() {
    const src = ReadableStream.from([0, 1, 2]);
    // deno-lint-ignore require-yield
    const transform = toTransformStream(function* (src) {
      src.getReader(); // lock the source stream to cause error at cancel
      throw new Error("foo");
    });

    await assertRejects(
      async () => await Array.fromAsync(src.pipeThrough(transform)),
      Error,
      "foo",
    );
  },
});
