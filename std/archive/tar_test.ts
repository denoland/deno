/**
 * Tar test
 *
 * **test summary**
 * - create a tar archive in memory containing output.txt and dir/tar.ts.
 * - read and deflate a tar archive containing output.txt
 *
 * **to run this test**
 * deno run --allow-read archive/tar_test.ts
 */
import { assertEquals, assert } from "../testing/asserts.ts";

import { resolve } from "../path/mod.ts";
import { Tar, Untar } from "./tar.ts";

const filePath = resolve("archive", "testdata", "example.txt");

interface TestEntry {
  name: string;
  content?: Uint8Array;
  filePath?: string;
}

async function createTar(entries: TestEntry[]): Promise<Tar> {
  const tar = new Tar();
  // put data on memory
  for (const file of entries) {
    let options;

    if (file.content) {
      options = {
        reader: new Deno.Buffer(file.content),
        contentSize: file.content.byteLength,
      };
    } else {
      options = { filePath: file.filePath };
    }

    await tar.append(file.name, options);
  }

  return tar;
}

Deno.test("createTarArchive", async function (): Promise<void> {
  // initialize
  const tar = new Tar();

  // put data on memory
  const content = new TextEncoder().encode("hello tar world!");
  await tar.append("output.txt", {
    reader: new Deno.Buffer(content),
    contentSize: content.byteLength,
  });

  // put a file
  await tar.append("dir/tar.ts", { filePath });

  // write tar data to a buffer
  const writer = new Deno.Buffer();
  const wrote = await Deno.copy(tar.getReader(), writer);

  /**
   * 3072 = 512 (header) + 512 (content) + 512 (header) + 512 (content)
   *       + 1024 (footer)
   */
  assertEquals(wrote, 3072);
});

Deno.test("deflateTarArchive", async function (): Promise<void> {
  const fileName = "output.txt";
  const text = "hello tar world!";

  // create a tar archive
  const tar = new Tar();
  const content = new TextEncoder().encode(text);
  await tar.append(fileName, {
    reader: new Deno.Buffer(content),
    contentSize: content.byteLength,
  });

  // read data from a tar archive
  const untar = new Untar(tar.getReader());
  const result = await untar.extract();
  assert(result !== null);
  const untarText = new TextDecoder("utf-8").decode(await Deno.readAll(result));

  assertEquals(await untar.extract(), null); // EOF
  // tests
  assertEquals(result.fileName, fileName);
  assertEquals(untarText, text);
});

Deno.test("appendFileWithLongNameToTarArchive", async function (): Promise<
  void
> {
  // 9 * 15 + 13 = 148 bytes
  const fileName = new Array(10).join("long-file-name/") + "file-name.txt";
  const text = "hello tar world!";

  // create a tar archive
  const tar = new Tar();
  const content = new TextEncoder().encode(text);
  await tar.append(fileName, {
    reader: new Deno.Buffer(content),
    contentSize: content.byteLength,
  });

  // read data from a tar archive
  const untar = new Untar(tar.getReader());
  const result = await untar.extract();
  assert(result !== null);
  const untarText = new TextDecoder("utf-8").decode(await Deno.readAll(result));

  // tests
  assertEquals(result.fileName, fileName);
  assertEquals(untarText, text);
});

Deno.test("untarAsyncIterator", async function (): Promise<void> {
  const entries: TestEntry[] = [
    {
      name: "output.txt",
      content: new TextEncoder().encode("hello tar world!"),
    },
    {
      name: "dir/tar.ts",
      filePath,
    },
  ];

  const tar = await createTar(entries);

  // read data from a tar archive
  const untar = new Untar(tar.getReader());

  for await (const entry of untar) {
    const expected = entries.shift();
    assert(expected);

    let content = expected.content;
    if (expected.filePath) {
      content = await Deno.readFile(expected.filePath);
    }

    assertEquals(content, await Deno.readAll(entry));
    assertEquals(expected.name, entry.fileName);
  }

  assertEquals(entries.length, 0);
});

Deno.test("untarAsyncIteratorWithoutReadingBody", async function (): Promise<
  void
> {
  const entries: TestEntry[] = [
    {
      name: "output.txt",
      content: new TextEncoder().encode("hello tar world!"),
    },
    {
      name: "dir/tar.ts",
      filePath,
    },
  ];

  const tar = await createTar(entries);

  // read data from a tar archive
  const untar = new Untar(tar.getReader());

  for await (const entry of untar) {
    const expected = entries.shift();
    assert(expected);
    assertEquals(expected.name, entry.fileName);
  }

  assertEquals(entries.length, 0);
});

Deno.test(
  "untarAsyncIteratorWithoutReadingBodyFromFileReader",
  async function (): Promise<void> {
    const entries: TestEntry[] = [
      {
        name: "output.txt",
        content: new TextEncoder().encode("hello tar world!"),
      },
      {
        name: "dir/tar.ts",
        filePath,
      },
    ];

    const outputFile = resolve("archive", "testdata", "test.tar");

    const tar = await createTar(entries);
    const file = await Deno.open(outputFile, { create: true, write: true });
    await Deno.copy(tar.getReader(), file);
    file.close();

    const reader = await Deno.open(outputFile, { read: true });
    // read data from a tar archive
    const untar = new Untar(reader);

    for await (const entry of untar) {
      const expected = entries.shift();
      assert(expected);
      assertEquals(expected.name, entry.fileName);
    }

    reader.close();
    await Deno.remove(outputFile);
    assertEquals(entries.length, 0);
  }
);

Deno.test("untarAsyncIteratorFromFileReader", async function (): Promise<void> {
  const entries: TestEntry[] = [
    {
      name: "output.txt",
      content: new TextEncoder().encode("hello tar world!"),
    },
    {
      name: "dir/tar.ts",
      filePath,
    },
  ];

  const outputFile = resolve("archive", "testdata", "test.tar");

  const tar = await createTar(entries);
  const file = await Deno.open(outputFile, { create: true, write: true });
  await Deno.copy(tar.getReader(), file);
  file.close();

  const reader = await Deno.open(outputFile, { read: true });
  // read data from a tar archive
  const untar = new Untar(reader);

  for await (const entry of untar) {
    const expected = entries.shift();
    assert(expected);

    let content = expected.content;
    if (expected.filePath) {
      content = await Deno.readFile(expected.filePath);
    }

    assertEquals(content, await Deno.readAll(entry));
    assertEquals(expected.name, entry.fileName);
  }

  reader.close();
  await Deno.remove(outputFile);
  assertEquals(entries.length, 0);
});

Deno.test(
  "untarAsyncIteratorReadingLessThanRecordSize",
  async function (): Promise<void> {
    // record size is 512
    const bufSizes = [1, 53, 256, 511];

    for (const bufSize of bufSizes) {
      const entries: TestEntry[] = [
        {
          name: "output.txt",
          content: new TextEncoder().encode("hello tar world!".repeat(100)),
        },
        // Need to test at least two files, to make sure the first entry doesn't over-read
        // Causing the next to fail with: chesum error
        {
          name: "deni.txt",
          content: new TextEncoder().encode("deno!".repeat(250)),
        },
      ];

      const tar = await createTar(entries);

      // read data from a tar archive
      const untar = new Untar(tar.getReader());

      for await (const entry of untar) {
        const expected = entries.shift();
        assert(expected);
        assertEquals(expected.name, entry.fileName);

        const writer = new Deno.Buffer();
        while (true) {
          const buf = new Uint8Array(bufSize);
          const n = await entry.read(buf);
          if (n === null) break;

          await writer.write(buf.subarray(0, n));
        }
        assertEquals(writer.bytes(), expected!.content);
      }

      assertEquals(entries.length, 0);
    }
  }
);

Deno.test("untarLinuxGeneratedTar", async function (): Promise<void> {
  const filePath = resolve("archive", "testdata", "deno.tar");
  const file = await Deno.open(filePath, { read: true });

  const expectedEntries = [
    {
      fileName: "archive/",
      fileSize: 0,
      fileMode: 509,
      mtime: 1591800767,
      uid: 1001,
      gid: 1001,
      owner: "deno",
      group: "deno",
      type: "directory",
    },
    {
      fileName: "archive/deno/",
      fileSize: 0,
      fileMode: 509,
      mtime: 1591799635,
      uid: 1001,
      gid: 1001,
      owner: "deno",
      group: "deno",
      type: "directory",
    },
    {
      fileName: "archive/deno/land/",
      fileSize: 0,
      fileMode: 509,
      mtime: 1591799660,
      uid: 1001,
      gid: 1001,
      owner: "deno",
      group: "deno",
      type: "directory",
    },
    {
      fileName: "archive/deno/land/land.txt",
      fileMode: 436,
      fileSize: 5,
      mtime: 1591799660,
      uid: 1001,
      gid: 1001,
      owner: "deno",
      group: "deno",
      type: "file",
      content: new TextEncoder().encode("land\n"),
    },
    {
      fileName: "archive/file.txt",
      fileMode: 436,
      fileSize: 5,
      mtime: 1591799626,
      uid: 1001,
      gid: 1001,
      owner: "deno",
      group: "deno",
      type: "file",
      content: new TextEncoder().encode("file\n"),
    },
    {
      fileName: "archive/deno.txt",
      fileMode: 436,
      fileSize: 5,
      mtime: 1591799642,
      uid: 1001,
      gid: 1001,
      owner: "deno",
      group: "deno",
      type: "file",
      content: new TextEncoder().encode("deno\n"),
    },
  ];

  const untar = new Untar(file);

  for await (const entry of untar) {
    const expected = expectedEntries.shift();
    assert(expected);
    const content = expected.content;
    delete expected.content;

    assertEquals(entry, expected);

    if (content) {
      assertEquals(content, await Deno.readAll(entry));
    }
  }

  file.close();
});
