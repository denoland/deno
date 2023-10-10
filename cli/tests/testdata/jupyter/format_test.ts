import vl from "npm:vega-lite-api";

const { format } = Deno.jupyter;
import { assertEquals } from "https://deno.land/std@0.201.0/assert/mod.ts";

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
