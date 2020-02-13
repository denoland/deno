import {
  AssertionError,
  assertThrowsAsync,
  assertEquals
} from "../testing/asserts";
import { writeTrailers } from "./http_io";
const { test, Buffer } = Deno;

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
