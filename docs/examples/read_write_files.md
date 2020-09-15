# Read and Write Files

## Concepts

- Deno's runtime API provides the
  [Deno.readTextFile](https://doc.deno.land/builtin/stable#Deno.readTextFile)
  and
  [Deno.writeTextFile](https://doc.deno.land/builtin/stable#Deno.writeTextFile)
  asynchronous functions for reading and writing entire text files
- Like many of Deno's APIs, synchronous alternatives are also available. See
  [Deno.readTextFileSync](https://doc.deno.land/builtin/stable#Deno.readTextFileSync)
  and
  [Deno.writeTextFileSync](https://doc.deno.land/builtin/stable#Deno.writeTextFileSync)
- Deno's [standard library]() provides additional functionality for file system
  access, for example reading and writing JSON
- Use `--allow-read` and `--allow-write` permissions to gain access to the file
  system

## Overview

Interacting with the filesystem to read and write files is a common requirement.
Deno provides a number of ways to do this via the
[standard library](https://deno.land/std) and the
[Deno runtime API](https://doc.deno.land/builtin/stable).

As highlighted in the [Fetch Data example](./fetch_data) Deno restricts access
to Input / Output by default for security reasons. Therefore when interacting
with the filesystem the `--allow-read` and `--allow-write` flags must be used
with the `deno run` command.

## Reading a text file

The Deno runtime API makes it possible to read text files via the
`readTextFile()` method, it just requires a path string or URL object. The
method returns a promise which provides access to the file's text data.

**Command:** `deno run --allow-read read.ts`

```typescript
/**
 * read.ts
 */
async function readFile(path: string): Promise<string> {
  return await Deno.readTextFile(new URL(path, import.meta.url));
}

const text = readFile("./people.json");

text.then((response) => console.log(response));

/**
 * Output:
 *
 * [
 *   {"id": 1, "name": "John", "age": 23},
 *   {"id": 2, "name": "Sandra", "age": 51},
 *   {"id": 5, "name": "Devika", "age": 11}
 * ]
 */
```

The Deno standard library enables more advanced interaction with the filesystem
and provides methods to read and parse files. The `readJson()` and
`readJsonSync()` methods allow developers to read and parse files containing
JSON. All these methods require is a valid file path string which can be
generated using the `fromFileUrl()` method.

In the example below the `readJsonSync()` method is used. For asynchronous
execution use the `readJson()` method.

Currently some of this functionality is marked as unstable so the `--unstable`
flag is required along with the `deno run` command.

**Command:** `deno run --unstable --allow-read read.ts`

```typescript
/**
 * read.ts
 */
import { readJsonSync } from "https://deno.land/std@$STD_VERSION/fs/mod.ts";
import { fromFileUrl } from "https://deno.land/std@$STD_VERSION/path/mod.ts";

function readJson(path: string): object {
  const file = fromFileUrl(new URL(path, import.meta.url));
  return readJsonSync(file) as object;
}

console.log(readJson("./people.json"));

/**
 * Output:
 *
 * [
 *   {"id": 1, "name": "John", "age": 23},
 *   {"id": 2, "name": "Sandra", "age": 51},
 *   {"id": 5, "name": "Devika", "age": 11}
 * ]
 */
```

## Writing a text file

The Deno runtime API allows developers to write text to files via the
`writeTextFile()` method. It just requires a file path and text string. The
method returns a promise which resolves when the file was successfully written.

To run the command the `--allow-write` flag must be supplied to the `deno run`
command.

**Command:** `deno run --allow-write write.ts`

```typescript
/**
 * write.ts
 */
async function writeFile(path: string, text: string): Promise<void> {
  return await Deno.writeTextFile(path, text);
}

const write = writeFile("./hello.txt", "Hello World!");

write.then(() => console.log("File written to ./hello.txt"));

/**
 * Output: File written to ./hello.txt
 */
```

The Deno standard library makes available more advanced features to write to the
filesystem. For instance it is possible to write an object literal to a JSON
file.

This requires a combination of the `ensureFile()`, `ensureFileSync()`,
`writeJson()` and `writeJsonSync()` methods. In the example below the
`ensureFileSync()` and the `writeJsonSync()` methods are used. The former checks
for the existence of a file, and if it doesn't exist creates it. The latter
method then writes the object to the file as JSON. If asynchronous execution is
required use the `ensureFile()` and `writeJson()` methods.

To execute the code the `deno run` command needs the unstable flag and both the
write and read flags.

**Command:** `deno run --allow-write --allow-read --unstable write.ts`

```typescript
/**
 * write.ts
 */
import {
  ensureFileSync,
  writeJsonSync,
} from "https://deno.land/std@$STD_VERSION/fs/mod.ts";

function writeJson(path: string, data: object): string {
  try {
    ensureFileSync(path);
    writeJsonSync(path, data);

    return "Written to " + path;
  } catch (e) {
    return e.message;
  }
}

console.log(writeJson("./data.json", { hello: "World" }));

/**
 * Output: Written to ./data.json
 */
```
