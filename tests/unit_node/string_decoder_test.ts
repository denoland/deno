// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals } from "@std/assert";
import { Buffer } from "node:buffer";
import { StringDecoder } from "node:string_decoder";

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
  },
});

Deno.test({
  name: "String decoder is encoding base64 correctly",
  fn() {
    let decoder;

    decoder = new StringDecoder("base64");
    assertEquals(decoder.write(Buffer.from("E1", "hex")), "");
    assertEquals(decoder.end(), "4Q==");

    decoder = new StringDecoder("base64");
    assertEquals(decoder.write(Buffer.from("E18B", "hex")), "");
    assertEquals(decoder.end(), "4Ys=");

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
    assertEquals(decoder.write(Buffer.from("EFBFBDE2", "hex")), "77+9");
    assertEquals(decoder.end(), "4g==");

    decoder = new StringDecoder("base64");
    assertEquals(decoder.write(Buffer.from("F1", "hex")), "");
    assertEquals(decoder.write(Buffer.from("41F2", "hex")), "8UHy");
    assertEquals(decoder.end(), "");
  },
});

Deno.test({
  name: "String decoder is encoding base64url correctly",
  fn() {
    let decoder;

    decoder = new StringDecoder("base64url");
    assertEquals(decoder.write(Buffer.from("E1", "hex")), "");
    assertEquals(decoder.end(), "4Q");

    decoder = new StringDecoder("base64url");
    assertEquals(decoder.write(Buffer.from("E18B", "hex")), "");
    assertEquals(decoder.end(), "4Ys");

    decoder = new StringDecoder("base64url");
    assertEquals(decoder.write(Buffer.from("\ufffd")), "77-9");
    assertEquals(decoder.end(), "");

    decoder = new StringDecoder("base64url");
    assertEquals(
      decoder.write(Buffer.from("\ufffd\ufffd\ufffd")),
      "77-977-977-9",
    );
    assertEquals(decoder.end(), "");

    decoder = new StringDecoder("base64url");
    assertEquals(decoder.write(Buffer.from("EFBFBDE2", "hex")), "77-9");
    assertEquals(decoder.end(), "4g");

    decoder = new StringDecoder("base64url");
    assertEquals(decoder.write(Buffer.from("F1", "hex")), "");
    assertEquals(decoder.write(Buffer.from("41F2", "hex")), "8UHy");
    assertEquals(decoder.end(), "");
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
  },
});

Deno.test({
  name:
    "String decoder with utf8 would handle incomplete character correctly when append",
  fn() {
    let decoder;
    const specialCharactersText = "不完全な文字のテスト";
    const encodedBuffer = Buffer.from(specialCharactersText);

    decoder = new StringDecoder("utf8");
    let str = "";
    str += decoder.write(encodedBuffer.slice(0, 4));
    assertEquals(str, "不");
    str += decoder.write(encodedBuffer.slice(4));
    assertEquals(str, "不完全な文字のテスト");

    decoder = new StringDecoder("utf8");
    str = "";
    str += decoder.write(encodedBuffer.slice(0, 4));
    str += decoder.write(encodedBuffer.slice(5));
    assertEquals(str, "不�全な文字のテスト");
  },
});

Deno.test({
  name: "String decoder would have default encoding option as utf8",
  fn() {
    let decoder;

    decoder = new StringDecoder();
    assertEquals(decoder.write(Buffer.from("E1", "hex")), "");
    assertEquals(decoder.end(), "\ufffd");

    decoder = new StringDecoder();
    assertEquals(decoder.write(Buffer.from("E18B", "hex")), "");
    assertEquals(decoder.end(), "\ufffd");

    decoder = new StringDecoder();
    assertEquals(decoder.write(Buffer.from("\ufffd")), "\ufffd");
    assertEquals(decoder.end(), "");

    decoder = new StringDecoder();
    assertEquals(
      decoder.write(Buffer.from("\ufffd\ufffd\ufffd")),
      "\ufffd\ufffd\ufffd",
    );
    assertEquals(decoder.end(), "");

    decoder = new StringDecoder();
    assertEquals(decoder.write(Buffer.from("EFBFBDE2", "hex")), "\ufffd");
    assertEquals(decoder.end(), "\ufffd");

    decoder = new StringDecoder();
    assertEquals(decoder.write(Buffer.from("F1", "hex")), "");
    assertEquals(decoder.write(Buffer.from("41F2", "hex")), "\ufffdA");
    assertEquals(decoder.end(), "\ufffd");
  },
});
