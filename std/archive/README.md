# Usage

## Tar

```ts
import { Tar } from "https://deno.land/std/archive/tar.ts";

const tar = new Tar();
const content = new TextEncoder().encode("Deno.land");
await tar.append("deno.txt", {
  reader: new Deno.Buffer(content),
  contentSize: content.byteLength,
});

// Or specifying a filePath
await tar.append("land.txt", {
  filePath: "./land.txt",
});

// use tar.getReader() to read the contents

const writer = await Deno.open("./out.tar", { write: true, create: true });
await Deno.copy(tar.getReader(), writer);
writer.close();
```

## Untar

```ts
import { Untar } from "https://deno.land/std/archive/tar.ts";
import { ensureFile } from "https://deno.land/std/fs/ensure_file.ts";
import { ensureDir } from "https://deno.land/std/fs/ensure_dir.ts";

const reader = await Deno.open("./out.tar", { read: true });
const untar = new Untar(reader);

for await (const entry of untar) {
  console.log(entry); // metadata
  /*
    fileName: "archive/deno.txt",
    fileMode: 33204,
    mtime: 1591657305,
    uid: 0,
    gid: 0,
    size: 24400,
    type: 'file'
  */

  if (entry.type === "directory") {
    await ensureDir(entry.fileName);
    continue;
  }

  await ensureFile(entry.fileName);
  const file = await Deno.open(entry.fileName, { write: true });
  // <entry> is a reader
  await Deno.copy(entry, file);
}
reader.close();
```
