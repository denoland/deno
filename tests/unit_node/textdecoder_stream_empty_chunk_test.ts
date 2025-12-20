// See: https://github.com/ExodusOSS/bytes/blob/4b758ba/tests/encoding/mistakes.test.js
import { assertEquals } from "../registry/jsr/@std/assert/1.0.0/assert_equals.ts";

Deno.test(
  "TextDecoder should handle empty chunk in stream mode (legacy encodings)",
  () => {
    // big5
    {
      const u8 = new Uint8Array([0x8e, 0xa0]);
      const str = new TextDecoder("big5").decode(u8);
      assertEquals(str, "\u594a");

      const d = new TextDecoder("big5");
      const chunks = [
        d.decode(u8.subarray(0, 1), { stream: true }),
        d.decode(u8.subarray(1), { stream: true }),
        d.decode(new Uint8Array(), { stream: true }),
        d.decode(),
      ];
      assertEquals(chunks.join(""), str);
    }

    // shift_jis
    {
      const u8 = new Uint8Array([0x81, 0x87]);
      const str = new TextDecoder("shift_jis").decode(u8);
      assertEquals(str, "\u3042");

      const d = new TextDecoder("shift_jis");
      const chunks = [
        d.decode(u8.subarray(0, 1), { stream: true }),
        d.decode(u8.subarray(1), { stream: true }),
        d.decode(new Uint8Array(), { stream: true }),
        d.decode(),
      ];
      assertEquals(chunks.join(""), str);
    }

    // euc-kr
    {
      const u8 = new Uint8Array([0xa4, 0xa2]);
      const str = new TextDecoder("euc-kr").decode(u8);
      assertEquals(str, "\uac02");

      const d = new TextDecoder("euc-kr");
      const chunks = [
        d.decode(u8.subarray(0, 1), { stream: true }),
        d.decode(u8.subarray(1), { stream: true }),
        d.decode(new Uint8Array(), { stream: true }),
        d.decode(),
      ];
      assertEquals(chunks.join(""), str);
    }
  },
);
