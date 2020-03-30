const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { pad } from "./pad.ts";

test(function padTest(): void {
  const expected1 = "**deno";
  const expected2 = "deno";
  const expected3 = "deno**";
  const expected4 = "denosorusrex";
  const expected5 = "denosorus";
  const expected6 = "sorusrex";
  const expected7 = "den...";
  const expected8 = "...rex";
  assertEquals(pad("deno", 6, { char: "*", side: "left" }), expected1);
  assertEquals(pad("deno", 4, { char: "*", side: "left" }), expected2);
  assertEquals(pad("deno", 6, { char: "*", side: "right" }), expected3);
  assertEquals(
    pad("denosorusrex", 4, {
      char: "*",
      side: "right",
      strict: false,
    }),
    expected4
  );
  assertEquals(
    pad("denosorusrex", 9, {
      char: "*",
      side: "left",
      strict: true,
      strictSide: "right",
    }),
    expected5
  );
  assertEquals(
    pad("denosorusrex", 8, {
      char: "*",
      side: "left",
      strict: true,
      strictSide: "left",
    }),
    expected6
  );
  assertEquals(
    pad("denosorusrex", 6, {
      char: "*",
      side: "left",
      strict: true,
      strictSide: "right",
      strictChar: "...",
    }),
    expected7
  );
  assertEquals(
    pad("denosorusrex", 6, {
      char: "*",
      side: "left",
      strict: true,
      strictSide: "left",
      strictChar: "...",
    }),
    expected8
  );
  assertEquals(
    pad("deno", 4, {
      char: "*",
      side: "left",
      strict: true,
      strictSide: "right",
      strictChar: "...",
    }),
    expected2
  );
});
