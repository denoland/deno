import {
  assert,
  assertEquals,
  assertStringIncludes,
  assertThrows,
} from "../../testing/asserts.ts";
import randomBytes, { MAX_RANDOM_VALUES, MAX_SIZE } from "./randomBytes.ts";

Deno.test("randomBytes sync works correctly", function () {
  assertEquals(randomBytes(0).length, 0, "len: " + 0);
  assertEquals(randomBytes(3).length, 3, "len: " + 3);
  assertEquals(randomBytes(30).length, 30, "len: " + 30);
  assertEquals(randomBytes(300).length, 300, "len: " + 300);
  assertEquals(
    randomBytes(17 + MAX_RANDOM_VALUES).length,
    17 + MAX_RANDOM_VALUES,
    "len: " + 17 + MAX_RANDOM_VALUES,
  );
  assertEquals(
    randomBytes(MAX_RANDOM_VALUES * 100).length,
    MAX_RANDOM_VALUES * 100,
    "len: " + MAX_RANDOM_VALUES * 100,
  );
  assertThrows(() => randomBytes(MAX_SIZE + 1));
  assertThrows(() => randomBytes(-1));
});

Deno.test("randomBytes async works correctly", function () {
  randomBytes(0, function (err, resp) {
    assert(!err);
    assertEquals(resp?.length, 0, "len: " + 0);
  });
  randomBytes(3, function (err, resp) {
    assert(!err);
    assertEquals(resp?.length, 3, "len: " + 3);
  });
  randomBytes(30, function (err, resp) {
    assert(!err);
    assertEquals(resp?.length, 30, "len: " + 30);
  });
  randomBytes(300, function (err, resp) {
    assert(!err);
    assertEquals(resp?.length, 300, "len: " + 300);
  });
  randomBytes(17 + MAX_RANDOM_VALUES, function (err, resp) {
    assert(!err);
    assertEquals(
      resp?.length,
      17 + MAX_RANDOM_VALUES,
      "len: " + 17 + MAX_RANDOM_VALUES,
    );
  });
  randomBytes(MAX_RANDOM_VALUES * 100, function (err, resp) {
    assert(!err);
    assertEquals(
      resp?.length,
      MAX_RANDOM_VALUES * 100,
      "len: " + MAX_RANDOM_VALUES * 100,
    );
  });
  assertThrows(() =>
    randomBytes(MAX_SIZE + 1, function (err) {
      //Shouldn't throw async
      assert(!err);
    })
  );
  assertThrows(() =>
    randomBytes(-1, function (err) {
      //Shouldn't throw async
      assert(!err);
    })
  );
});

Deno.test("[std/node/crypto] randomBytes callback isn't called twice if error is thrown", async () => {
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "--no-check",
      `
      import randomBytes from "${
        new URL("./randomBytes.ts", import.meta.url).href
      }";

      randomBytes(0, (err) => {
        // If the bug is present and the callback is called again with an error,
        // don't throw another error, so if the subprocess fails we know it had the correct behaviour.
        if (!err) throw new Error("success");
      });`,
    ],
    stderr: "piped",
  });
  const status = await p.status();
  const stderr = new TextDecoder().decode(await Deno.readAll(p.stderr));
  p.close();
  p.stderr.close();
  assert(!status.success);
  assertStringIncludes(stderr, "Error: success");
});
