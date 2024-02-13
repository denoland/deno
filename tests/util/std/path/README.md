# path

path module is made to provide helpers to manipulate the path.

## Usage

```ts
import * as path from "https://deno.land/std@$STD_VERSION/path/mod.ts";
```

Codes in the following example uses POSIX path but it automatically use Windows
path on Windows. Use methods under `posix` or `win32` object instead to handle
non platform specific path like:

```ts
import { posix, win32 } from "https://deno.land/std@$STD_VERSION/path/mod.ts";
const p1 = posix.fromFileUrl("file:///home/foo");
const p2 = win32.fromFileUrl("file:///home/foo");
console.log(p1); // "/home/foo"
console.log(p2); // "\\home\\foo"
```

### basename

Return the last portion of a `path`. Trailing directory separators are ignored.

```ts
import { basename } from "https://deno.land/std@$STD_VERSION/path/mod.ts";
const p = basename("./deno/std/path/mod.ts");
console.log(p); // "mod.ts"
```

### dirname

Return the directory name of a `path`.

```ts
import { dirname } from "https://deno.land/std@$STD_VERSION/path/mod.ts";
const p = dirname("./deno/std/path/mod.ts");
console.log(p); // "./deno/std/path"
```

### extname

Return the extension of the `path`.

```ts
import { extname } from "https://deno.land/std@$STD_VERSION/path/mod.ts";
const p = extname("./deno/std/path/mod.ts");
console.log(p); // ".ts"
```

### format

Generate a path from `FormatInputPathObject` object.

```ts
import { format } from "https://deno.land/std@$STD_VERSION/path/mod.ts";
const p = format({
  root: "/",
  dir: "/home/user/dir",
  ext: ".html",
  name: "index",
});
console.log(p); // "/home/user/dir/index.html"
```

### fromFileUrl

Converts a file URL to a path string.

```ts
import { fromFileUrl } from "https://deno.land/std@$STD_VERSION/path/mod.ts";
const p = fromFileUrl("file:///home/foo");
console.log(p); // "/home/foo"
```

### isAbsolute

Verifies whether provided path is absolute

```ts
import { isAbsolute } from "https://deno.land/std@$STD_VERSION/path/mod.ts";
console.log(isAbsolute("/home/foo")); // true
console.log(isAbsolute("foo")); // false
```

### join

Join all given a sequence of `paths`,then normalizes the resulting path.

```ts
import { join } from "https://deno.land/std@$STD_VERSION/path/mod.ts";
const p = join("foo", "bar");
console.log(p); // "foo/bar"
```

### normalize

Normalize the `path`, resolving `'..'` and `'.'` segments.

```ts
import { normalize } from "https://deno.land/std@$STD_VERSION/path/mod.ts";
const p = normalize("/home/foo/bar/../hoge/./piyo");
console.log(p); // "/home/foo/hoge/piyo"
```

### parse

Return a `ParsedPath` object of the `path`.

```ts
import { parse } from "https://deno.land/std@$STD_VERSION/path/mod.ts";
const p = parse("/home/user/dir/index.html");
console.log(p);
/*
 * {
 *   root: "/",
 *   dir: "/home/user/dir",
 *   base: "index.html",
 *   ext: ".html",
 *   name: "index",
 * }
 */
```

### relative

Return the relative path from `from` to `to` based on current working directory.

```ts
import { relative } from "https://deno.land/std@$STD_VERSION/path/mod.ts";
const p = relative("/var/lib", "/var/apache");
console.log(p); // "../apache"
```

### resolve

Resolves `pathSegments` into an absolute path.

```ts
import { resolve } from "https://deno.land/std@$STD_VERSION/path/mod.ts";
const p = resolve("/var/lib", "../", "file/");
console.log(p); // "/var/file"
```

### toFileUrl

Converts a path string to a file URL.

```ts
import { toFileUrl } from "https://deno.land/std@$STD_VERSION/path/mod.ts";
const p = toFileUrl("/home/foo");
console.log(p);
/*
 * URL {
 *   href: "file:///home/foo",
 *   origin: "null",
 *   protocol: "file:",
 *   username: "",
 *   password: "",
 *   host: "",
 *   hostname: "",
 *   port: "",
 *   pathname: "/home/foo",
 *   hash: "",
 *   search: ""
 * }
 */
```

### toNamespacedPath

Resolves path to a namespace path

```ts
import { toNamespacedPath } from "https://deno.land/std@$STD_VERSION/path/mod.ts";
const p = toNamespacedPath("/home/foo");
console.log(p); // "/home/foo"
```

### common

Determines the common path from a set of paths, using an optional separator,
which defaults to the OS default separator.

```ts
import { common } from "https://deno.land/std@$STD_VERSION/path/mod.ts";
const p = common([
  "./deno/std/path/mod.ts",
  "./deno/std/fs/mod.ts",
]);
console.log(p); // "./deno/std/"
```

### globToRegExp

Generate a regex based on glob pattern and options This was meant to be using
the `fs.walk` function but can be used anywhere else.

```ts
import { globToRegExp } from "https://deno.land/std@$STD_VERSION/path/glob.ts";

globToRegExp("foo/**/*.json", {
  extended: true,
  globstar: true,
  caseInsensitive: false,
}); // returns the regex to find all .json files in the folder foo.
```
