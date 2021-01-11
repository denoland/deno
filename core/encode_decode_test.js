// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}

function assertArrayEquals(a1, a2) {
  if (a1.length !== a2.length) throw Error("assert");

  for (const index in a1) {
    if (a1[index] !== a2[index]) {
      throw Error("assert");
    }
  }
}

function main() {
  // deno-fmt-ignore
  const fixture1 = [
    0xf0, 0x9d, 0x93, 0xbd,
    0xf0, 0x9d, 0x93, 0xae,
    0xf0, 0x9d, 0x94, 0x81,
    0xf0, 0x9d, 0x93, 0xbd
  ];
  // deno-fmt-ignore
  const fixture2 = [
    72, 101, 108, 108,
    111, 32, 239, 191,
    189, 239, 191, 189,
    32, 87, 111, 114,
    108, 100
  ];

  const empty = Deno.core.encode("");
  if (empty.length !== 0) throw new Error("assert");

  assertArrayEquals(Array.from(Deno.core.encode("𝓽𝓮𝔁𝓽")), fixture1);
  assertArrayEquals(
    Array.from(Deno.core.encode("Hello \udc12\ud834 World")),
    fixture2,
  );

  const emptyBuf = Deno.core.decode(new Uint8Array(0));
  if (emptyBuf !== "") throw new Error("assert");

  assert(Deno.core.decode(new Uint8Array(fixture1)) === "𝓽𝓮𝔁𝓽");
  assert(Deno.core.decode(new Uint8Array(fixture2)) === "Hello �� World");

  // See https://github.com/denoland/deno/issues/6649
  let thrown = false;
  try {
    Deno.core.decode(new Uint8Array(2 ** 29));
  } catch (e) {
    thrown = true;
    assert(e instanceof RangeError);
    assert(e.message === "string too long");
  }
  assert(thrown);
}

main();
