import {
  AssertionError,
  assertThrowsAsync,
  assertEquals
} from "../testing/asserts.ts";
import { bodyReader, writeTrailers, readTrailers } from "./io.ts";
import { encode, decode } from "../strings/mod.ts";
import { BufReader } from "../io/bufio.ts";
import { chunkedBodyReader } from "./io.ts";
const { test, Buffer } = Deno;

test("bodyReader", async () => {
  const text = "Hello, Deno";
  const r = bodyReader(text.length, new BufReader(new Buffer(encode(text))));
  assertEquals(decode(await Deno.readAll(r)), text);
});
function chunkify(n: number, char: string): string {
  const v = Array.from({ length: n })
    .map(() => `${char}`)
    .join("");
  return `${n.toString(16)}\r\n${v}\r\n`;
}
test("chunkedBodyReader", async () => {
  const body = [
    chunkify(3, "a"),
    chunkify(5, "b"),
    chunkify(11, "c"),
    chunkify(22, "d"),
    chunkify(0, "")
  ].join("");
  const h = new Headers();
  const r = chunkedBodyReader(h, new BufReader(new Buffer(encode(body))));
  let result: number | Deno.EOF;
  // Use small buffer as some chunks exceed buffer size
  const buf = new Uint8Array(5);
  const dest = new Buffer();
  while ((result = await r.read(buf)) !== Deno.EOF) {
    const len = Math.min(buf.byteLength, result);
    await dest.write(buf.subarray(0, len));
  }
  const exp = "aaabbbbbcccccccccccdddddddddddddddddddddd";
  assertEquals(dest.toString(), exp);
});

test("chunkedBodyReader with trailers", async () => {
  const body = [
    chunkify(3, "a"),
    chunkify(5, "b"),
    chunkify(11, "c"),
    chunkify(22, "d"),
    chunkify(0, ""),
    "deno: land\r\n",
    "node: js\r\n",
    "\r\n"
  ].join("");
  const h = new Headers({
    trailer: "deno,node"
  });
  const r = chunkedBodyReader(h, new BufReader(new Buffer(encode(body))));
  assertEquals(h.has("trailer"), true);
  assertEquals(h.has("deno"), false);
  assertEquals(h.has("node"), false);
  const act = decode(await Deno.readAll(r));
  const exp = "aaabbbbbcccccccccccdddddddddddddddddddddd";
  assertEquals(act, exp);
  assertEquals(h.has("trailer"), false);
  assertEquals(h.get("deno"), "land");
  assertEquals(h.get("node"), "js");
});

test("readTrailers", async () => {
  const h = new Headers({
    trailer: "deno,node"
  });
  const trailer = ["deno: land", "node: js", "", ""].join("\r\n");
  await readTrailers(h, new BufReader(new Buffer(encode(trailer))));
  assertEquals(h.has("trailer"), false);
  assertEquals(h.get("deno"), "land");
  assertEquals(h.get("node"), "js");
});

test("readTrailer should throw if undeclared headers found in trailer", async () => {
  const patterns = [
    ["deno,node", "deno: land\r\nnode: js\r\ngo: lang\r\n\r\n"],
    ["deno", "node: js\r\n\r\n"],
    ["deno", "node:js\r\ngo: lang\r\n\r\n"]
  ];
  for (const [header, trailer] of patterns) {
    const h = new Headers({
      trailer: header
    });
    await assertThrowsAsync(
      async () => {
        await readTrailers(h, new BufReader(new Buffer(encode(trailer))));
      },
      Error,
      "Undeclared trailer field"
    );
  }
});

test("readTrailer should throw if trailer contains prohibited fields", async () => {
  for (const f of ["content-length", "trailer", "transfer-encoding"]) {
    const h = new Headers({
      trailer: f
    });
    await assertThrowsAsync(
      async () => {
        await readTrailers(h, new BufReader(new Buffer()));
      },
      Error,
      "Prohibited field for trailer"
    );
  }
});

test("writeTrailer", async () => {
  const w = new Buffer();
  await writeTrailers(
    w,
    new Headers({ "transfer-encoding": "chunked", trailer: "deno,node" }),
    new Headers({ deno: "land", node: "js" })
  );
  assertEquals(w.toString(), "deno: land\r\nnode: js\r\n\r\n");
});

test("writeTrailer should throw", async () => {
  const w = new Buffer();
  await assertThrowsAsync(
    () => {
      return writeTrailers(w, new Headers(), new Headers());
    },
    Error,
    'must have "trailer"'
  );
  await assertThrowsAsync(
    () => {
      return writeTrailers(w, new Headers({ trailer: "deno" }), new Headers());
    },
    Error,
    "only allowed"
  );
  for (const f of ["content-length", "trailer", "transfer-encoding"]) {
    await assertThrowsAsync(
      () => {
        return writeTrailers(
          w,
          new Headers({ "transfer-encoding": "chunked", trailer: f }),
          new Headers({ [f]: "1" })
        );
      },
      AssertionError,
      "prohibited"
    );
  }
  await assertThrowsAsync(
    () => {
      return writeTrailers(
        w,
        new Headers({ "transfer-encoding": "chunked", trailer: "deno" }),
        new Headers({ node: "js" })
      );
    },
    AssertionError,
    "Not trailer"
  );
});
