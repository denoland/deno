import { assert, assertEquals, fail } from "../../testing/asserts.ts";
import { open, openSync } from "./_fs_open.ts";
import { parse, join } from "../../path/mod.ts";
import { existsSync } from "../../fs/mod.ts";

const temp_dir = parse(Deno.makeTempFileSync()).dir;

Deno.test({
  name: "ASYNC: open file",
  async fn() {
    const file = Deno.makeTempFileSync();
    await new Promise<number>((resolve, reject) => {
      open(file, (err, fd) => {
        if (err) reject(err);
        resolve(fd);
      });
    })
      .then((fd) => assert(Deno.resources()[fd]))
      .catch(() => fail())
      .finally(() => Deno.removeSync(file));
  },
});

Deno.test({
  name: "SYNC: open file",
  fn() {
    const file = Deno.makeTempFileSync();
    const fd = openSync(file);
    assert(Deno.resources()[fd]);
    Deno.removeSync(file);
  },
});

Deno.test({
  name: "open with flag 'a'",
  fn() {
    const file = join(temp_dir, "some_random_file");
    const fd = openSync(file, "a");
    assertEquals(typeof fd, "number");
    assertEquals(existsSync(file), true);
    assert(Deno.resources()[fd]);
    Deno.removeSync(file);
  },
});

Deno.test({
  name: "open with flag 'ax'",
  fn() {
    const file = Deno.makeTempFileSync();
    let err;
    try {
      openSync(file, "ax");
    } catch (error) {
      err = error;
    }
    Deno.removeSync(file);
    assert(err);
  },
});

Deno.test({
  name: "open with flag 'a+'",
  fn() {
    const file = join(temp_dir, "some_random_file2");
    const fd = openSync(file, "a+");
    assertEquals(typeof fd, "number");
    assertEquals(existsSync(file), true);
    Deno.removeSync(file);
  },
});

Deno.test({
  name: "open with flag 'ax+'",
  fn() {
    const file = Deno.makeTempFileSync();
    let err;
    try {
      openSync(file, "ax+");
    } catch (error) {
      err = error;
    }
    Deno.removeSync(file);
    assert(err);
  },
});

Deno.test({
  name: "open with flag 'r'",
  fn() {
    const file = join(temp_dir, "some_random_file3");
    let err;
    try {
      openSync(file, "r");
    } catch (error) {
      err = error;
    }
    Deno.removeSync(file);
    assert(err);
  },
});

Deno.test({
  name: "open with flag 'r+'",
  fn() {
    const file = join(temp_dir, "some_random_file4");
    let err;
    try {
      openSync(file, "r+");
    } catch (error) {
      err = error;
    }
    Deno.removeSync(err);
    assert(err);
  },
});

Deno.test({
  name: "open with flag 'w'",
  fn() {
    const file = Deno.makeTempFileSync();
    Deno.writeTextFileSync(file, "hi there");
    const fd = openSync(file, "w");
    assertEquals(typeof fd, "number");
    assertEquals(Deno.readTextFileSync(file), "");
    Deno.removeSync(file);

    const file2 = join(temp_dir, "some_random_file5");
    const fd2 = openSync(file2, "w");
    assertEquals(typeof fd2, "number");
    assertEquals(existsSync(file2), true);
    Deno.removeSync(file2);
  },
});

Deno.test({
  name: "open with flag 'wx'",
  fn() {
    const file = Deno.makeTempFileSync();
    Deno.writeTextFileSync(file, "hi there");
    const fd = openSync(file, "wx");
    assertEquals(typeof fd, "number");
    assertEquals(Deno.readTextFileSync(file), "");
    Deno.removeSync(file);

    const file2 = Deno.makeTempFileSync();
    let err;
    try {
      openSync(file2, "wx");
    } catch (error) {
      err = error;
    }
  },
});

Deno.test({
  name: "open with flag 'w+'",
  fn() {
    const file = Deno.makeTempFileSync();
    Deno.writeTextFileSync(file, "hi there");
    const fd = openSync(file, "w+");
    assertEquals(typeof fd, "number");
    assertEquals(Deno.readTextFileSync(file), "");
    Deno.removeSync(file);

    const file2 = join(temp_dir, "some_random_file6");
    openSync(file2, "w+");
    assertEquals(typeof fd, "number");
    assertEquals(existsSync(file2), true);
    Deno.removeSync(file);
  },
});

Deno.test({
  name: "open with flag 'wx+'",
  fn() {
    const file = Deno.makeTempFileSync();
    Deno.writeTextFileSync(file, "hi there");
    const fd = openSync(file, "wx+");
    assertEquals(typeof fd, "number");
    assertEquals(Deno.readTextFileSync(file), "");
    Deno.removeSync(file);

    const file2 = Deno.makeTempFileSync();
    let err;
    try {
      openSync(file2, "wx+");
    } catch (error) {
      err = error;
    }
    Deno.removeSync(file2);
  },
});
