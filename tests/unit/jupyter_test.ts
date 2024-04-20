// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertThrows } from "./test_util.ts";

// @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
const format = Deno[Deno.internal].jupyter.formatInner;

Deno.test("Deno.jupyter is not available", () => {
  assertThrows(
    () => Deno.jupyter,
    "Deno.jupyter is only available in `deno jupyter` subcommand.",
  );
});

export async function assertFormattedAs(obj: unknown, result: object) {
  const formatted = await format(obj);
  assertEquals(formatted, result);
}

Deno.test("display(canvas) creates a PNG", async () => {
  // Let's make a fake Canvas with a fake Data URL
  class FakeCanvas {
    toDataURL() {
      return "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUAAAAFCAYAAACNbyblAAAAAXNSR0IArs4c6QAAAARzQklUCAgICHwIZIgAAAAVSURBVAiZY/zPwPCfAQ0woQtQQRAAzqkCCB/D3o0AAAAASUVORK5CYII=";
    }
  }
  const canvas = new FakeCanvas();

  await assertFormattedAs(canvas, {
    "image/png":
      "iVBORw0KGgoAAAANSUhEUgAAAAUAAAAFCAYAAACNbyblAAAAAXNSR0IArs4c6QAAAARzQklUCAgICHwIZIgAAAAVSURBVAiZY/zPwPCfAQ0woQtQQRAAzqkCCB/D3o0AAAAASUVORK5CYII=",
  });
});

Deno.test(
  "class with a Symbol.for('Jupyter.display') function gets displayed",
  async () => {
    class Example {
      x: number;

      constructor(x: number) {
        this.x = x;
      }

      [Symbol.for("Jupyter.display")]() {
        return { "application/json": { x: this.x } };
      }
    }

    const example = new Example(5);

    // Now to check on the broadcast call being made
    await assertFormattedAs(example, { "application/json": { x: 5 } });
  },
);

Deno.test(
  "class with an async Symbol.for('Jupyter.display') function gets displayed",
  async () => {
    class Example {
      x: number;

      constructor(x: number) {
        this.x = x;
      }

      async [Symbol.for("Jupyter.display")]() {
        await new Promise((resolve) => setTimeout(resolve, 0));

        return { "application/json": { x: this.x } };
      }
    }

    const example = new Example(3);

    // Now to check on the broadcast call being made
    await assertFormattedAs(example, { "application/json": { x: 3 } });
  },
);
