// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../../testing/asserts.ts";
import { mul32, mul64 } from "./util.ts";

Deno.test("[hash/fnv/util] mul32", () => {
  assertEquals(mul32(0xffffffff, 0xffffffff), 1);
  assertEquals(mul32(0x12345678, 0xdeadbeef), 0x5621ca08);
  assertEquals(mul32(0xf626f430, 0xff7469f1), 0x2a939130);
  assertEquals(mul32(0x543f9412, 0x8a4aa84f), 0x39fe818e);
  assertEquals(mul32(0x8ee170d1, 0x2fbbb9ec), 0x6a0609ac);
  assertEquals(mul32(0xea3b3a14, 0xa397bd0a), 0xddfd08c8);
  assertEquals(mul32(0x93f8536b, 0xa79e3c04), 0xcc7861ac);
  assertEquals(mul32(0xf97dab98, 0xed526241), 0x2348c198);
  assertEquals(mul32(0x35500191, 0xd5012447), 0xaff9d337);
  assertEquals(mul32(0x471dde47, 0xaaa4950c), 0x4341be54);
  assertEquals(mul32(0xd633970d, 0xa9bc2bcd), 0xb43b2469);
  assertEquals(mul32(0xc60898cc, 0xbfe7dcc4), 0x15f84c30);
});

Deno.test("[hash/fnv/util] mul64", () => {
  assertEquals(mul64([0xffffffff, 0xffffffff], [0xffffffff, 0xffffffff]), [
    0,
    1,
  ]);
  assertEquals(mul64([0x12345678, 0xdeadbeef], [0xcafebabe, 0xbaadf00d]), [
    0xc801c86b,
    0xdf55c223,
  ]);
  assertEquals(mul64([0xdc479aed, 0x24bc71a3], [0x543717c1, 0x4b6056b9]), [
    0x56c7ec8f,
    0x387ae0cb,
  ]);
  assertEquals(mul64([0xb84936ae, 0xb84becd2], [0x2864edd1, 0x14ee13cc]), [
    0xd87e9171,
    0x12504d58,
  ]);
  assertEquals(mul64([0xb0b73e95, 0x3f5cc701], [0x6c7b30b8, 0xcd7f0f9e]), [
    0x570551ee,
    0x116ae19e,
  ]);
  assertEquals(mul64([0xc237b433, 0x160b50bf], [0x3f937c23, 0xf26175f7]), [
    0x48a1d118,
    0x97313349,
  ]);
  assertEquals(mul64([0x386242fd, 0x6baa0fc0], [0xf81f7e23, 0xbe172381]), [
    0x4799f2a3,
    0x6b192fc0,
  ]);
  assertEquals(mul64([0x5afc8714, 0x902180d1], [0xa7068c96, 0xb859bb4d]), [
    0xb4589d29,
    0xd3d569dd,
  ]);
  assertEquals(mul64([0xb4e86a68, 0x619bee92], [0xd67560fa, 0x736982a7]), [
    0x72c73b5d,
    0x4bc0c53e,
  ]);
  assertEquals(mul64([0xfc8b5561, 0xbf91d6d5], [0x2bcb029a, 0xa144ead3]), [
    0x2da439a7,
    0x3926c38f,
  ]);
  assertEquals(mul64([0x47b62fae, 0xffe8cb4c], [0xbda77111, 0x6cad4968]), [
    0x9d9b7832,
    0xcae742e0,
  ]);
  assertEquals(mul64([0xc9160fc1, 0xd96e085b], [0x3adfd031, 0x3f75e557]), [
    0xe4d0bf23,
    0x88753ded,
  ]);
});
