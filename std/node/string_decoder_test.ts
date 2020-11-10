// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { Buffer } from "./buffer.ts";
import { StringDecoder } from "./string_decoder.ts";

Deno.test({
  name: "String decoder is encoding utf8 correctly",
  fn() {
    let decoder;

    decoder = new StringDecoder("utf8");
    assertEquals(decoder.write(Buffer.from("E1", "hex")), "");
    assertEquals(decoder.end(), "\ufffd");

    decoder = new StringDecoder("utf8");
    assertEquals(decoder.write(Buffer.from("E18B", "hex")), "");
    assertEquals(decoder.end(), "\ufffd");

    decoder = new StringDecoder("utf8");
    assertEquals(decoder.write(Buffer.from("\ufffd")), "\ufffd");
    assertEquals(decoder.end(), "");

    decoder = new StringDecoder("utf8");
    assertEquals(
      decoder.write(Buffer.from("\ufffd\ufffd\ufffd")),
      "\ufffd\ufffd\ufffd",
    );
    assertEquals(decoder.end(), "");

    decoder = new StringDecoder("utf8");
    assertEquals(decoder.write(Buffer.from("EFBFBDE2", "hex")), "\ufffd");
    assertEquals(decoder.end(), "\ufffd");

    decoder = new StringDecoder("utf8");
    assertEquals(decoder.write(Buffer.from("F1", "hex")), "");
    assertEquals(decoder.write(Buffer.from("41F2", "hex")), "\ufffdA");
    assertEquals(decoder.end(), "\ufffd");

    decoder = new StringDecoder("utf8");
    assertEquals(decoder.text(Buffer.from([0x41]), 2), "");
  },
});

Deno.test({
  name: "String decoder is encoding base64 correctly",
  fn() {
    let decoder;

    decoder = new StringDecoder("base64");
    assertEquals(decoder.write(Buffer.from("E1", "hex")), "4Q==");
    assertEquals(decoder.end(), "4QAA");

    decoder = new StringDecoder("base64");
    assertEquals(decoder.write(Buffer.from("E18B", "hex")), "4Ys=");
    assertEquals(decoder.end(), "4YsA");

    decoder = new StringDecoder("base64");
    assertEquals(decoder.write(Buffer.from("\ufffd")), "77+9");
    assertEquals(decoder.end(), "");

    decoder = new StringDecoder("base64");
    assertEquals(
      decoder.write(Buffer.from("\ufffd\ufffd\ufffd")),
      "77+977+977+9",
    );
    assertEquals(decoder.end(), "");

    decoder = new StringDecoder("base64");
    assertEquals(decoder.write(Buffer.from("EFBFBDE2", "hex")), "77+94g==");
    assertEquals(decoder.end(), "4gAA");

    decoder = new StringDecoder("base64");
    assertEquals(decoder.write(Buffer.from("F1", "hex")), "8Q==");
    assertEquals(decoder.write(Buffer.from("41F2", "hex")), "8UHy");
    assertEquals(decoder.end(), "");

    decoder = new StringDecoder("base64");
    assertEquals(decoder.text(Buffer.from([0x41]), 2), "QQ==");
  },
});

Deno.test({
  name: "String decoder is encoding hex correctly",
  fn() {
    let decoder;

    decoder = new StringDecoder("hex");
    assertEquals(decoder.write(Buffer.from("E1", "hex")), "e1");
    assertEquals(decoder.end(), "");

    decoder = new StringDecoder("hex");
    assertEquals(decoder.write(Buffer.from("E18B", "hex")), "e18b");
    assertEquals(decoder.end(), "");

    decoder = new StringDecoder("hex");
    assertEquals(decoder.write(Buffer.from("\ufffd")), "efbfbd");
    assertEquals(decoder.end(), "");

    decoder = new StringDecoder("hex");
    assertEquals(
      decoder.write(Buffer.from("\ufffd\ufffd\ufffd")),
      "efbfbdefbfbdefbfbd",
    );
    assertEquals(decoder.end(), "");

    decoder = new StringDecoder("hex");
    assertEquals(decoder.write(Buffer.from("EFBFBDE2", "hex")), "efbfbde2");
    assertEquals(decoder.end(), "");

    decoder = new StringDecoder("hex");
    assertEquals(decoder.write(Buffer.from("F1", "hex")), "f1");
    assertEquals(decoder.write(Buffer.from("41F2", "hex")), "41f2");
    assertEquals(decoder.end(), "");

    decoder = new StringDecoder("hex");
    assertEquals(decoder.text(Buffer.from([0x41]), 2), "");
  },
});
