# media_types

A module that assists in resolving media types and extensions. It consumes the
[mime-db](https://github.com/jshttp/mime-db) and provides API access to the
information.

## Usage

### `lookup(path)`

Lookup the content type associated with a file. The path can be just the
extension or the full path name. If the content type cannot be determined the
function returns `undefined`:

```ts
import { lookup } from "https://deno.land/std/media_types/mod.ts";

lookup("json"); // "application/json"
lookup(".md"); // "text/markdown"
lookup("folder/file.js"); // "application/javascript"
lookup("folder/.htaccess"); // undefined
```

### `contentType(type)`

Return a full `Content-Type` header value for a given content type or
extension. When an extension is used, `lookup()` is used to resolve the
content type first. A default charset is added if not present. The
function will return `undefined` if the content type cannot be resolved:

```ts
import { contentType } from "https://deno.land/std/media_types/mod.ts";
import * as path from "https://deno.land/std/path/mod.ts";

contentType("markdown"); // "text/markdown; charset=utf-8"
contentType("file.json"); // "application/json; charset=utf-8"
contentType("text/html"); // "text/html; charset=utf-8"
contentType("text/html; charset=iso-8859-1"); // "text/html; charset=iso-8859-1"

contentType(path.extname("/path/to/file.json")); // "application/json; charset=utf-8"
```

### `extension(type)`

Return a default extension for a given content type. If there is not an
appropriate extension, `undefined` is returned:

```ts
import { extension } from "https://deno.land/std/media_types/mod.ts";

extension("application/octet-stream"); // "bin"
```

### `charset(type)`

Lookup the implied default charset for a given content type. If the content
type cannot be resolved, `undefined` is returned:

```ts
import { charset } from "https://deno.land/std/media_types/mod.ts";

charset("text/markdown"); // "UTF-8"
```

### `extensions`

A `Map` of extensions by content type, in priority order:

```ts
import { extensions } from "https://deno.land/std/media_types/mod.ts";

extensions.get("application/javascript"); // [ "js", "mjs" ]
```

### `types`

A `Map` of content types by extension:

```ts
import { types } from "https://deno.land/std/media_types/mod.ts";

types.get("ts"); // "application/javascript"
```

---

Adapted from [mime-type](https://github.com/jshttp/mime-types).

MIT License.
