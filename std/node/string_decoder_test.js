import { assertEquals } from "../testing/asserts.ts";
import Buffer from "./buffer.ts";
import { StringDecoder } from "./string_decoder.js";

let decoder;

Deno.test({
  name: "String decoder is encoding utf8 correctly",
  fn() {
    decoder = new StringDecoder('utf8');
    assertEquals(decoder.write(Buffer.from('E1', 'hex')), '');
    assertEquals(decoder.end(), '\ufffd');

    decoder = new StringDecoder('utf8');
    assertEquals(decoder.write(Buffer.from('E18B', 'hex')), '');
    assertEquals(decoder.end(), '\ufffd');

    decoder = new StringDecoder('utf8');
    assertEquals(decoder.write(Buffer.from('\ufffd')), '\ufffd');
    assertEquals(decoder.end(), '');

    decoder = new StringDecoder('utf8');
    assertEquals(decoder.write(Buffer.from('\ufffd\ufffd\ufffd')), '\ufffd\ufffd\ufffd');
    assertEquals(decoder.end(), '');

    decoder = new StringDecoder('utf8');
    assertEquals(decoder.write(Buffer.from('EFBFBDE2', 'hex')), '\ufffd');
    assertEquals(decoder.end(), '\ufffd');

    decoder = new StringDecoder('utf8');
    assertEquals(decoder.write(Buffer.from('F1', 'hex')), '');
    assertEquals(decoder.write(Buffer.from('41F2', 'hex')), '\ufffdA');
    assertEquals(decoder.end(), '\ufffd');

    decoder = new StringDecoder('utf8');
    assertEquals(decoder.text(Buffer.from([0x41]), 2), '');
  },
});
