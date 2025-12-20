import { assertEquals } from "../registry/jsr/@std/assert/1.0.0/assert_equals.ts";

Deno.test("TextDecoder stream:true and empty chunk", () => {
  const encodings = ["big5", "shift_jis", "euc-kr", "utf-8"];
  for (const enc of encodings) {
    const dec = new TextDecoder(enc);
    // stream:true and empty input
    const result = dec.decode(new Uint8Array([]), { stream: true });
    assertEquals(result, "", `${enc} should return empty string for stream:true and empty chunk`);
  }
});
