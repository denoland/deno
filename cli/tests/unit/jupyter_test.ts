// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertThrows } from "./test_util.ts";

import {
  assertSpyCall,
  spy,
} from "https://deno.land/std@0.202.0/testing/mock.ts";

Deno.test("Deno.jupyter is not available", () => {
  assertThrows(
    () => Deno.jupyter,
    "Deno.jupyter is only available in `deno jupyter` subcommand.",
  );
});

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
  Deno[Deno.internal].enableJupyter();

  const { display } = Deno.jupyter;

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
  // Let's make a fake Canvas with a fake Data URL
  class FakeCanvas {
    toDataURL() {
      return "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUAAAAFCAYAAACNbyblAAAAAXNSR0IArs4c6QAAAARzQklUCAgICHwIZIgAAAAVSURBVAiZY/zPwPCfAQ0woQtQQRAAzqkCCB/D3o0AAAAASUVORK5CYII=";
    }
  }

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
