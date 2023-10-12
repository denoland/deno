// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertThrows, assertEquals } from "./test_util.ts";

import vl from "npm:vega-lite-api";

const { format } = Deno.jupyter;

Deno.test("Deno.jupyter is not available", () => {
  assertThrows(
    () => Deno.jupyter,
    "Deno.jupyter is only available in `deno jupyter` subcommand.",
  );
});

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


Deno.test("display(vl_plot) returns a MediaBundle", () => {
  const plot = vl
    .markBar({ tooltip: true })
    .data([
      { a: "A", b: 28 },
      { a: "B", b: 55 },
      { a: "C", b: 43 },
      { a: "D", b: 91 },
      { a: "E", b: 81 },
      { a: "F", b: 53 },
      { a: "G", b: 19 },
      { a: "H", b: 87 },
      { a: "I", b: 52 },
    ])
    .encode(
      vl.x().fieldQ("b"),
      vl.y().fieldN("a"),
      vl.tooltip([vl.fieldQ("b"), vl.fieldN("a")]),
    );

  const bundle = format(plot);

  const expected = {
    "application/vnd.vegalite.v5+json": {
      $schema: "https://vega.github.io/schema/vega-lite/v5.json",
      mark: { type: "bar", tooltip: true },
      data: {
        values: [
          { a: "A", b: 28 },
          { a: "B", b: 55 },
          { a: "C", b: 43 },
          { a: "D", b: 91 },
          { a: "E", b: 81 },
          { a: "F", b: 53 },
          { a: "G", b: 19 },
          { a: "H", b: 87 },
          { a: "I", b: 52 },
        ],
      },
      encoding: {
        x: { field: "b", type: "quantitative" },
        y: { field: "a", type: "nominal" },
        tooltip: [
          { field: "b", type: "quantitative" },
          { field: "a", type: "nominal" },
        ],
      },
    },
  };

  assertEquals(bundle, expected);
});


