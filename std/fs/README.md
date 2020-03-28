# fs

fs module is made to provide helpers to manipulate the filesystem.

## Usage

All the following modules are exposed in `mod.ts`

### emptyDir

Ensures that a directory is empty. Deletes directory contents if the directory
is not empty. If the directory does not exist, it is created. The directory
itself is not deleted.

```ts
import { emptyDir, emptyDirSync } from "https://deno.land/std/fs/mod.ts";

emptyDir("./foo"); // returns a promise
emptyDirSync("./foo"); // void
```

### ensureDir

Ensures that the directory exists. If the directory structure does not exist, it
is created. Like mkdir -p.

```ts
import { ensureDir, ensureDirSync } from "https://deno.land/std/fs/mod.ts";

ensureDir("./bar"); // returns a promise
ensureDirSync("./ensureDirSync"); // void
```

### ensureFile

Ensures that the file exists. If the file that is requested to be created is in
directories that do not exist, these directories are created. If the file
already exists, it is **NOT MODIFIED**.

```ts
import { ensureFile, ensureFileSync } from "https://deno.land/std/fs/mod.ts";

ensureFile("./folder/targetFile.dat"); // returns promise
ensureFileSync("./folder/targetFile.dat"); // void
```

### ensureSymlink

Ensures that the link exists. If the directory structure does not exist, it is
created.

```ts
import {
  ensureSymlink,
  ensureSymlinkSync,
} from "https://deno.land/std/fs/mod.ts";

ensureSymlink(
  "./folder/targetFile.dat",
  "./folder/targetFile.link.dat",
  "file"
); // returns promise
ensureSymlinkSync(
  "./folder/targetFile.dat",
  "./folder/targetFile.link.dat",
  "file"
); // void
```

### eol

Detects and format the passed string for the targeted End Of Line character.

```ts
import { format, detect, EOL } from "https://deno.land/std/fs/mod.ts";

const CRLFinput = "deno\r\nis not\r\nnode";
const Mixedinput = "deno\nis not\r\nnode";
const LFinput = "deno\nis not\nnode";
const NoNLinput = "deno is not node";

detect(LFinput); // output EOL.LF
detect(CRLFinput); // output EOL.CRLF
detect(Mixedinput); // output EOL.CRLF
detect(NoNLinput); // output null

format(CRLFinput, EOL.LF); // output "deno\nis not\nnode"
...
```

### exists

Test whether or not the given path exists by checking with the file system

```ts
import { exists, existsSync } from "https://deno.land/std/fs/mod.ts";

exists("./foo"); // returns a Promise<boolean>
existsSync("./foo"); // returns boolean
```

### globToRegExp

Generate a regex based on glob pattern and options This was meant to be using
the the `fs.walk` function but can be used anywhere else.

```ts
import { globToRegExp } from "https://deno.land/std/fs/mod.ts";

globToRegExp("foo/**/*.json", {
  flags: "g",
  extended: true,
  globstar: true,
}); // returns the regex to find all .json files in the folder foo
```

### move

Moves a file or directory. Overwrites it if option provided

```ts
import { move, moveSync } from "https://deno.land/std/fs/mod.ts";

move("./foo", "./bar"); // returns a promise
moveSync("./foo", "./bar"); // void
moveSync("./foo", "./existingFolder", { overwrite: true });
// Will overwrite existingFolder
```

### copy

copy a file or directory. Overwrites it if option provided

```ts
import { copy, copySync } from "https://deno.land/std/fs/mod.ts";

copy("./foo", "./bar"); // returns a promise
copySync("./foo", "./bar"); // void
copySync("./foo", "./existingFolder", { overwrite: true });
// Will overwrite existingFolder
```

### readJson

Reads a JSON file and then parses it into an object

```ts
import { readJson, readJsonSync } from "https://deno.land/std/fs/mod.ts";

const f = await readJson("./foo.json");
const foo = readJsonSync("./foo.json");
```

### walk

Iterate all files in a directory recursively.

```ts
import { walk, walkSync } from "https://deno.land/std/fs/mod.ts";

for (const fileInfo of walkSync(".")) {
  console.log(fileInfo.filename);
}

// Async
async function printFilesNames() {
  for await (const fileInfo of walk()) {
    console.log(fileInfo.filename);
  }
}

printFilesNames().then(() => console.log("Done!"));
```

### writeJson

Writes an object to a JSON file.

**WriteJsonOptions**

- replacer : An array of strings and numbers that acts as a approved list for
  selecting the object properties that will be stringified.
- space : Adds indentation, white space, and line break characters to the
  return-value JSON text to make it easier to read.

```ts
import { writeJson, writeJsonSync } from "https://deno.land/std/fs/mod.ts";

writeJson("./target.dat", { foo: "bar" }, { spaces: 2 }); // returns a promise
writeJsonSync("./target.dat", { foo: "bar" }, { replacer: ["foo"] }); // void
```

### readFileStr

Read file and output it as a string.

**ReadOptions**

- encoding : The encoding to read file. lowercased.

```ts
import { readFileStr, readFileStrSync } from "https://deno.land/std/fs/mod.ts";

readFileStr("./target.dat", { encoding: "utf8" }); // returns a promise
readFileStrSync("./target.dat", { encoding: "utf8" }); // void
```

### writeFileStr

Write the string to file.

```ts
import {
  writeFileStr,
  writeFileStrSync,
} from "https://deno.land/std/fs/mod.ts";

writeFileStr("./target.dat", "file content"); // returns a promise
writeFileStrSync("./target.dat", "file content"); // void
```
