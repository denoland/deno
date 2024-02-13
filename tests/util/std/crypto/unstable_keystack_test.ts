// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals } from "../assert/mod.ts";

import { KeyStack } from "./unstable_keystack.ts";

Deno.test({
  name: "keyStack.sign() - single key",
  async fn() {
    const keys = ["hello"];
    const keyStack = new KeyStack(keys);
    const actual = await keyStack.sign("world");
    const expected = "8ayXAutfryPKKRpNxG3t3u4qeMza8KQSvtdxTP_7HMQ";
    assertEquals(actual, expected);
  },
});

Deno.test({
  name: "keyStack.sign() - two keys, first key used",
  async fn() {
    const keys = ["hello", "world"];
    const keyStack = new KeyStack(keys);
    const actual = await keyStack.sign("world");
    const expected = "8ayXAutfryPKKRpNxG3t3u4qeMza8KQSvtdxTP_7HMQ";
    assertEquals(actual, expected);
  },
});

Deno.test({
  name: "keyStack.verify() - single key",
  async fn() {
    const keys = ["hello"];
    const keyStack = new KeyStack(keys);
    const digest = await keyStack.sign("world");
    assert(await keyStack.verify("world", digest));
  },
});

Deno.test({
  name: "keyStack.verify() - single key verify invalid",
  async fn() {
    const keys = ["hello"];
    const keyStack = new KeyStack(keys);
    const digest = await keyStack.sign("world");
    assert(!await keyStack.verify("worlds", digest));
  },
});

Deno.test({
  name: "keyStack.verify() - two keys",
  async fn() {
    const keys = ["hello", "world"];
    const keyStack = new KeyStack(keys);
    const digest = await keyStack.sign("world");
    assert(await keyStack.verify("world", digest));
  },
});

Deno.test({
  name: "keyStack.verify() - unshift key",
  async fn() {
    const keys = ["hello"];
    const keyStack = new KeyStack(keys);
    const digest = await keyStack.sign("world");
    keys.unshift("world");
    assertEquals(keys, ["world", "hello"]);
    assert(await keyStack.verify("world", digest));
  },
});

Deno.test({
  name: "keyStack.verify() - shift key",
  async fn() {
    const keys = ["hello", "world"];
    const keyStack = new KeyStack(keys);
    const digest = await keyStack.sign("world");
    assertEquals(keys.shift(), "hello");
    assertEquals(keys, ["world"]);
    assert(!await keyStack.verify("world", digest));
  },
});

Deno.test({
  name: "keyStack.indexOf() - single key",
  async fn() {
    const keys = ["hello"];
    const keyStack = new KeyStack(keys);
    assertEquals(
      await keyStack.indexOf(
        "world",
        "8ayXAutfryPKKRpNxG3t3u4qeMza8KQSvtdxTP_7HMQ",
      ),
      0,
    );
  },
});

Deno.test({
  name: "keyStack.indexOf() - two keys index 0",
  async fn() {
    const keys = ["hello", "world"];
    const keyStack = new KeyStack(keys);
    assertEquals(
      await keyStack.indexOf(
        "world",
        "8ayXAutfryPKKRpNxG3t3u4qeMza8KQSvtdxTP_7HMQ",
      ),
      0,
    );
  },
});

Deno.test({
  name: "keyStack.indexOf() - two keys index 1",
  async fn() {
    const keys = ["world", "hello"];
    const keyStack = new KeyStack(keys);
    assertEquals(
      await keyStack.indexOf(
        "world",
        "8ayXAutfryPKKRpNxG3t3u4qeMza8KQSvtdxTP_7HMQ",
      ),
      1,
    );
  },
});

Deno.test({
  name: "keyStack.indexOf() - two keys not found",
  async fn() {
    const keys = ["world", "hello"];
    const keyStack = new KeyStack(keys);
    assertEquals(
      await keyStack.indexOf(
        "hello",
        "8ayXAutfryPKKRpNxG3t3u4qeMza8KQSvtdxTP_7HMQ",
      ),
      -1,
    );
  },
});

Deno.test({
  name: "keyStack - number array key",
  async fn() {
    const keys = [[212, 213]];
    const keyStack = new KeyStack(keys);
    assert(await keyStack.verify("hello", await keyStack.sign("hello")));
  },
});

Deno.test({
  name: "keyStack - Uint8Array key",
  async fn() {
    const keys = [new Uint8Array([212, 213])];
    const keyStack = new KeyStack(keys);
    assert(await keyStack.verify("hello", await keyStack.sign("hello")));
  },
});

Deno.test({
  name: "keyStack - ArrayBuffer key",
  async fn() {
    const key = new ArrayBuffer(2);
    const dataView = new DataView(key);
    dataView.setInt8(0, 212);
    dataView.setInt8(1, 213);
    const keys = [key];
    const keyStack = new KeyStack(keys);
    assert(await keyStack.verify("hello", await keyStack.sign("hello")));
  },
});

Deno.test({
  name: "keyStack - number array data",
  async fn() {
    const keys = [[212, 213]];
    const keyStack = new KeyStack(keys);
    assert(await keyStack.verify([212, 213], await keyStack.sign([212, 213])));
  },
});

Deno.test({
  name: "keyStack - Uint8Array data",
  async fn() {
    const keys = [[212, 213]];
    const keyStack = new KeyStack(keys);
    assert(
      await keyStack.verify(
        new Uint8Array([212, 213]),
        await keyStack.sign(new Uint8Array([212, 213])),
      ),
    );
  },
});

Deno.test({
  name: "keyStack - ArrayBuffer data",
  async fn() {
    const keys = [[212, 213]];
    const keyStack = new KeyStack(keys);
    const data1 = new ArrayBuffer(2);
    const dataView1 = new DataView(data1);
    dataView1.setInt8(0, 212);
    dataView1.setInt8(1, 213);
    const data2 = new ArrayBuffer(2);
    const dataView2 = new DataView(data2);
    dataView2.setInt8(0, 212);
    dataView2.setInt8(1, 213);
    assert(await keyStack.verify(data2, await keyStack.sign(data1)));
  },
});

Deno.test({
  name: "keyStack - user iterable keys",
  async fn() {
    const keys = new Set(["hello", "world"]);
    const keyStack = new KeyStack(keys);
    const actual = await keyStack.sign("world");
    const expected = "8ayXAutfryPKKRpNxG3t3u4qeMza8KQSvtdxTP_7HMQ";
    assertEquals(actual, expected);
  },
});

Deno.test({
  name: "KeyStack - inspecting",
  fn() {
    assertEquals(
      Deno.inspect(new KeyStack(["abcdef"])),
      `KeyStack { length: 1 }`,
    );
  },
});
