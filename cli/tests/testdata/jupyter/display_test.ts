import {
  assertSpyCall,
  spy,
} from "https://deno.land/std@0.202.0/testing/mock.ts";

import { createCanvas } from "https://deno.land/x/canvas@v1.4.1/mod.ts";

const { display } = Deno.jupyter;

export async function fakeBroadcast(
  _msgType: string,
  _content: Record<string, unknown>,
  _extras?: {
    metadata?: Record<string, unknown>;
    buffers?: ArrayBuffer[];
    [key: string]: unknown;
  },
): Promise<void> {
  await Promise.resolve();
}

export async function assertDisplayedAs(obj: unknown, result: object) {
  const originalBroadcast = Deno.jupyter.broadcast;
  const mockedBroadcast = spy(fakeBroadcast);

  Deno.jupyter.broadcast = mockedBroadcast;

  await display(obj);

  assertSpyCall(mockedBroadcast, 0, {
    args: ["display_data", { data: result, metadata: {}, transient: {} }],
  });

  Deno.jupyter.broadcast = originalBroadcast;
}

Deno.test("display(canvas) creates a PNG", async () => {
  const canvas = createCanvas(5, 5);
  const ctx = canvas.getContext("2d");
  ctx.fillStyle = "red";
  ctx.fillRect(0, 0, 5, 5);

  await assertDisplayedAs(canvas, {
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
    await assertDisplayedAs(example, { "application/json": { x: 5 } });
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
    await assertDisplayedAs(example, { "application/json": { x: 3 } });
  },
);
