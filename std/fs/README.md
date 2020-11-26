# fs

fs module is made to provide helpers to manipulate the filesystem.

## Usage

Most of the following modules are exposed in `mod.ts`. This feature is currently
<b>unstable</b>. To enable it use `deno run --unstable`.

### emptyDir

Ensures that a directory is empty. Deletes directory contents if the directory
is not empty. If the directory does not exist, it is created. The directory
itself is not deleted.

```ts
import {
  emptyDir,
  emptyDirSync,
} from "https://deno.land/std@$STD_VERSION/fs/mod.ts";

emptyDir("./foo"); // returns a promise
emptyDirSync("./foo"); // void
```

### ensureDir

Ensures that the directory exists. If the directory structure does not exist, it
is created. Like `mkdir -p`.

```ts
import {
  ensureDir,
  ensureDirSync,
} from "https://deno.land/std@$STD_VERSION/fs/mod.ts";

ensureDir("./bar"); // returns a promise
ensureDirSync("./ensureDirSync"); // void
```

### ensureFile

Ensures that the file exists. If the file that is requested to be created is in
directories that do not exist, these directories are created. If the file
already exists, it is **NOT MODIFIED**.

```ts
import {
  ensureFile,
  ensureFileSync,
} from "https://deno.land/std@$STD_VERSION/fs/mod.ts";

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
} from "https://deno.land/std@$STD_VERSION/fs/mod.ts";

ensureSymlink("./folder/targetFile.dat", "./folder/targetFile.link.dat"); // returns promise
ensureSymlinkSync("./folder/targetFile.dat", "./folder/targetFile.link.dat"); // void
```

### EOL

Detects and format the passed string for the targeted End Of Line character.

```ts
import { format, detect, EOL } from "https://deno.land/std@$STD_VERSION/fs/mod.ts";

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

Test whether or not the given path exists by checking with the file system.

```ts
import {
  exists,
  existsSync,
} from "https://deno.land/std@$STD_VERSION/fs/mod.ts";

exists("./foo"); // returns a Promise<boolean>
existsSync("./foo"); // returns boolean
```

### move

Moves a file or directory. Overwrites it if option provided.

```ts
import { move, moveSync } from "https://deno.land/std@$STD_VERSION/fs/mod.ts";

move("./foo", "./bar"); // returns a promise
moveSync("./foo", "./bar"); // void
moveSync("./foo", "./existingFolder", { overwrite: true });
// Will overwrite existingFolder
```

### copy

copy a file or directory. Overwrites it if option provided.

```ts
import { copy, copySync } from "https://deno.land/std@$STD_VERSION/fs/mod.ts";

copy("./foo", "./bar"); // returns a promise
copySync("./foo", "./bar"); // void
copySync("./foo", "./existingFolder", { overwrite: true });
// Will overwrite existingFolder
```

### walk

Iterate all files in a directory recursively.

```ts
import { walk, walkSync } from "https://deno.land/std@$STD_VERSION/fs/mod.ts";

for (const entry of walkSync(".")) {
  console.log(entry.path);
}

// Async
async function printFilesNames() {
  for await (const entry of walk(".")) {
    console.log(entry.path);
  }
}

printFilesNames().then(() => console.log("Done!"));
```

### expandGlob

Expand the glob string from the specified `root` directory and yield each result
as a `WalkEntry` object.

```ts
import { expandGlob } from "https://deno.land/std@$STD_VERSION/fs/mod.ts";

for await (const file of expandGlob("**/*.ts")) {
  console.log(file);
}
```

### expandGlobSync

Synchronous version of `expandGlob()`.

```ts
import { expandGlobSync } from "https://deno.land/std@$STD_VERSION/fs/mod.ts";

for (const file of expandGlobSync("**/*.ts")) {
  console.log(file);
}
```
