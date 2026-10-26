// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals, assertRejects, assertThrows } from "@std/assert";
import { Buffer } from "node:buffer";
import { openAsBlob } from "node:fs";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";

async function withFile(
  contents: string | Uint8Array,
  fn: (path: string) => Promise<void>,
) {
  const dir = await Deno.makeTempDir();
  const path = join(dir, "blob.dat");
  try {
    await Deno.writeFile(
      path,
      typeof contents === "string"
        ? new TextEncoder().encode(contents)
        : contents,
    );
    await fn(path);
  } finally {
    await Deno.remove(dir, { recursive: true });
  }
}

Deno.test("openAsBlob rejects files modified after creation", async () => {
  await withFile("original", async (path) => {
    const blob = await openAsBlob(path);
    await Deno.writeTextFile(path, "modified contents");
    const error = await assertRejects(() => blob.arrayBuffer(), DOMException);
    assertEquals(error.name, "NotReadableError");
  });
});

Deno.test("openAsBlob reads nested and concurrent ranges", async () => {
  const contents = Uint8Array.from({ length: 200_000 }, (_, i) => i % 251);
  await withFile(contents, async (path) => {
    const blob = await openAsBlob(path);
    const nested = blob.slice(1000, 180_000).slice(65_000, 66_000);
    const [first, second, third] = await Promise.all([
      blob.slice(65_530, 65_550).bytes(),
      nested.bytes(),
      blob.slice(-100).bytes(),
    ]);
    assertEquals(first, contents.slice(65_530, 65_550));
    assertEquals(second, contents.slice(66_000, 67_000));
    assertEquals(third, contents.slice(-100));
    assertEquals(blob.size, contents.length);
    assertEquals(await blob.slice(100, 50).bytes(), new Uint8Array());
    assertEquals(await blob.slice(1_000_000).text(), "");
  });
});

Deno.test("openAsBlob streams bounded chunks and supports BYOB", async () => {
  const contents = new Uint8Array(200_000).fill(42);
  await withFile(contents, async (path) => {
    const blob = await openAsBlob(path);
    let total = 0;
    for await (const chunk of blob.stream()) {
      assert(chunk.byteLength <= 65_536);
      assert(chunk.every((byte) => byte === 42));
      total += chunk.byteLength;
    }
    assertEquals(total, contents.length);
    const reader = blob.slice(10, 50).stream().getReader({ mode: "byob" });
    try {
      const first = await reader.read(new Uint8Array(100));
      assertEquals(first.done, false);
      assertEquals(first.value, contents.slice(10, 50));
      assertEquals((await reader.read(new Uint8Array(10))).done, true);
    } finally {
      await reader.cancel();
    }
  });
});

Deno.test("openAsBlob cancellation closes only its reader", async () => {
  await withFile(new Uint8Array(150_000).fill(7), async (path) => {
    const blob = await openAsBlob(path);
    const reader = blob.stream().getReader();
    assertEquals((await reader.read()).value?.length, 65_536);
    await reader.cancel();
    assertEquals(await blob.slice(-3).bytes(), new Uint8Array([7, 7, 7]));
  });
});

Deno.test("openAsBlob detects changes between stream reads", async () => {
  await withFile(new Uint8Array(150_000).fill(7), async (path) => {
    const blob = await openAsBlob(path);
    const reader = blob.stream().getReader();
    await reader.read();
    await Deno.truncate(path, 100);
    const error = await assertRejects(() => reader.read(), DOMException);
    assertEquals(error.name, "NotReadableError");
    reader.releaseLock();
  });
});

Deno.test("openAsBlob detects same-size modifications", async () => {
  await withFile("original", async (path) => {
    const blob = await openAsBlob(path);
    await Deno.writeTextFile(path, "changed!");
    const later = new Date(Date.now() + 10_000);
    await Deno.utime(path, later, later);
    const error = await assertRejects(() => blob.text(), DOMException);
    assertEquals(error.name, "NotReadableError");
  });
});

Deno.test("openAsBlob rejects removed and replaced files", async () => {
  for (const replace of [false, true]) {
    await withFile("original", async (path) => {
      const blob = await openAsBlob(path);
      await Deno.remove(path);
      if (replace) await Deno.writeTextFile(path, "replacement");
      const error = await assertRejects(() => blob.bytes(), DOMException);
      assertEquals(error.name, "NotReadableError");
    });
  }
});

Deno.test("openAsBlob composes with Blob, File and FormData", async () => {
  await withFile("original", async (path) => {
    const fileBlob = await openAsBlob(path);
    const composite = new Blob(["prefix:", fileBlob.slice(1, 5), ":suffix"]);
    assertEquals(await composite.text(), "prefix:rigi:suffix");
    const file = new File([fileBlob], "contents.txt", { type: "text/plain" });
    const form = new FormData();
    form.set("file", file);
    const response = new Response(form);
    const parsed = await response.formData();
    assertEquals(await (parsed.get("file") as File).text(), "original");
    for (const value of [composite, file]) {
      const error = assertThrows(() => structuredClone(value), DOMException);
      assertEquals(error.name, "DataCloneError");
    }
  });
});

Deno.test("openAsBlob object URLs preserve sliced file parts", async () => {
  await withFile("0123456789", async (path) => {
    const blob = await openAsBlob(path, { type: "text/plain" });
    const url = URL.createObjectURL(new Blob(["before", blob.slice(2, 6)]));
    try {
      const response = await fetch(url);
      assertEquals(await response.text(), "before2345");
      await Deno.writeTextFile(path, "modified contents");
      const changedResponse = await fetch(url);
      const error = await assertRejects(
        () => changedResponse.text(),
        DOMException,
      );
      assertEquals(error.name, "NotReadableError");
    } finally {
      URL.revokeObjectURL(url);
    }
  });
});

Deno.test("openAsBlob accepts Buffer and URL paths", async () => {
  await withFile("original", async (path) => {
    for (const value of [Buffer.from(path), pathToFileURL(path)]) {
      const blob = await openAsBlob(value, { type: "TEXT/PLAIN" });
      assertEquals(blob.type, "text/plain");
      assertEquals(await blob.text(), "original");
    }
  });
});

Deno.test("openAsBlob handles an empty file without leaking a reader", async () => {
  await withFile("", async (path) => {
    const blob = await openAsBlob(path);
    assertEquals(blob.size, 0);
    assertEquals(await blob.bytes(), new Uint8Array());
    await Deno.writeTextFile(path, "changed");
    const error = await assertRejects(() => blob.text(), DOMException);
    assertEquals(error.name, "NotReadableError");
  });
});

Deno.test("openAsBlob reports directory errors when read", async () => {
  await withFile("original", async (path) => {
    const directory = dirname(path);
    const blob = await openAsBlob(directory);
    assertEquals(blob.size, (await Deno.stat(directory)).size);
    const error = await assertRejects(() => blob.bytes(), DOMException);
    assertEquals(error.name, "NotReadableError");
  });
});

Deno.test({
  name: "openAsBlob checks permissions when reopening a file",
  permissions: { read: true, write: true },
  async fn() {
    await withFile("original", async (path) => {
      const blob = await openAsBlob(path);
      await Deno.permissions.revoke({ name: "read" });
      const error = await assertRejects(() => blob.text(), DOMException);
      assertEquals(error.name, "NotReadableError");
    });
  },
});

Deno.test({
  name: "openAsBlob does not keep descriptors open for unread blobs",
  ignore: Deno.build.os !== "linux",
  async fn() {
    await withFile("original", async (path) => {
      const countDescriptors = () =>
        [...Deno.readDirSync("/proc/self/fd")].length;
      const before = countDescriptors();
      const blobs = await Promise.all(
        Array.from({ length: 32 }, () => openAsBlob(path)),
      );
      assert(countDescriptors() <= before + 8);
      assertEquals(blobs.map((blob) => blob.size), Array(32).fill(8));
      assertEquals(await blobs[0].text(), "original");
    });
  },
});
