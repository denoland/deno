import { assert } from "../testing/asserts";

const { test } = Deno;

/*
 * This file is a temporary test file to understand better the behaviour on the Windows CI which is failing
 * for Deno.open() with permission denied.
 */

test({
  name: "Async open current directory",
  async fn() {
    if (Deno.build.os == "win") {
      const fileInfo: Deno.File = await Deno.open(".");
      assert(fileInfo);
    }
  }
});

test({
  name: "Async open Program Files",
  async fn() {
    if (Deno.build.os == "win") {
      const fileInfo: Deno.File = await Deno.open("C:\\Program\\ Files");
      assert(fileInfo);
    }
  }
});

test({
  name: "Async open Users",
  async fn() {
    if (Deno.build.os == "win") {
      const fileInfo: Deno.File = await Deno.open("C:\\Users");
      assert(fileInfo);
    }
  }
});

test({
  name: "Sync open current dir",
  async fn() {
    if (Deno.build.os == "win") {
      const fileInfo: Deno.File = Deno.openSync(".");
      assert(fileInfo);
    }
  }
});

test({
  name: "Sync open Users",
  async fn() {
    if (Deno.build.os == "win") {
      const fileInfo: Deno.File = Deno.openSync("C:\\Users");
      assert(fileInfo);
    }
  }
});

test({
  name: "Async open current directory with options",
  async fn() {
    if (Deno.build.os == "win") {
      const fileInfo: Deno.File = await Deno.open(".", {
        read: true,
        write: false
      });
      assert(fileInfo);
    }
  }
});

test({
  name: "Async open Program Files with options",
  async fn() {
    if (Deno.build.os == "win") {
      const fileInfo: Deno.File = await Deno.open("C:\\Program\\ Files", {
        read: false,
        write: true
      });
      assert(fileInfo);
    }
  }
});

test({
  name: "Async open Users with options",
  async fn() {
    if (Deno.build.os == "win") {
      const fileInfo: Deno.File = await Deno.open("C:\\Users", {
        read: true,
        write: true
      });
      assert(fileInfo);
    }
  }
});

test({
  name: "Sync open current dir with options",
  async fn() {
    if (Deno.build.os == "win") {
      const fileInfo: Deno.File = Deno.openSync(".", {
        read: true,
        write: false
      });
      assert(fileInfo);
    }
  }
});

test({
  name: "Sync open Users with options",
  async fn() {
    if (Deno.build.os == "win") {
      const fileInfo: Deno.File = Deno.openSync("C:\\Users", {
        read: true,
        write: true
      });
      assert(fileInfo);
    }
  }
});
