## Permission APIs

> This API is unstable. Learn more about
> [unstable features](../runtime/stability.md).

Permissions are granted from the CLI when running the `deno` command. User code
will often assume its own set of required permissions, but there is no guarantee
during execution that the set of _granted_ permissions will align with this.

In some cases, ensuring a fault-tolerant program requires a way to interact with
the permission system at runtime.

### Permission descriptors

On the CLI, read permission for `/foo/bar` is represented as
`--allow-read=/foo/bar`. In runtime JS, it is represented as the following:

```ts
const desc = { name: "read", path: "/foo/bar" };
```

Other examples:

```ts
// Global write permission.
const desc1 = { name: "write" };

// Write permission to `$PWD/foo/bar`.
const desc2 = { name: "write", path: "foo/bar" };

// Global net permission.
const desc3 = { name: "net" };

// Net permission to 127.0.0.1:8000.
const desc4 = { name: "net", url: "127.0.0.1:8000" };

// High-resolution time permission.
const desc5 = { name: "hrtime" };
```

### Query permissions

Check, by descriptor, if a permission is granted or not.

```ts
// deno run --unstable --allow-read=/foo main.ts

const desc1 = { name: "read", path: "/foo" };
console.log(await Deno.permissions.query(desc1));
// PermissionStatus { state: "granted" }

const desc2 = { name: "read", path: "/foo/bar" };
console.log(await Deno.permissions.query(desc2));
// PermissionStatus { state: "granted" }

const desc3 = { name: "read", path: "/bar" };
console.log(await Deno.permissions.query(desc3));
// PermissionStatus { state: "prompt" }
```

### Permission states

A permission state can be either "granted", "prompt" or "denied". Permissions
which have been granted from the CLI will query to `{ state: "granted" }`. Those
which have not been granted query to `{ state: "prompt" }` by default, while
`{ state: "denied" }` reserved for those which have been explicitly refused.
This will come up in [Request permissions](#request-permissions).

### Permission strength

The intuitive understanding behind the result of the second query in
[Query permissions](#query-permissions) is that read access was granted to
`/foo` and `/foo/bar` is within `/foo` so `/foo/bar` is allowed to be read.

We can also say that `desc1` is
_[stronger than](https://www.w3.org/TR/permissions/#ref-for-permissiondescriptor-stronger-than)_
`desc2`. This means that for any set of CLI-granted permissions:

1. If `desc1` queries to `{ state: "granted" }` then so must `desc2`.
2. If `desc2` queries to `{ state: "denied" }` then so must `desc1`.

More examples:

```ts
const desc1 = { name: "write" };
// is stronger than
const desc2 = { name: "write", path: "/foo" };

const desc3 = { name: "net" };
// is stronger than
const desc4 = { name: "net", url: "127.0.0.1:8000" };
```

### Request permissions

Request an ungranted permission from the user via CLI prompt.

```ts
// deno run --unstable main.ts

const desc1 = { name: "read", path: "/foo" };
const status1 = await Deno.permissions.request(desc1);
// ⚠️ Deno requests read access to "/foo". Grant? [g/d (g = grant, d = deny)] g
console.log(status1);
// PermissionStatus { state: "granted" }

const desc2 = { name: "read", path: "/bar" };
const status2 = await Deno.permissions.request(desc2);
// ⚠️ Deno requests read access to "/bar". Grant? [g/d (g = grant, d = deny)] d
console.log(status2);
// PermissionStatus { state: "denied" }
```

If the current permission state is "prompt", a prompt will appear on the user's
terminal asking them if they would like to grant the request. The request for
`desc1` was granted so its new status is returned and execution will continue as
if `--allow-read=/foo` was specified on the CLI. The request for `desc2` was
denied so its permission state is downgraded from "prompt" to "denied".

If the current permission state is already either "granted" or "denied", the
request will behave like a query and just return the current status. This
prevents prompts both for already granted permissions and previously denied
requests.

### Revoke permissions

Downgrade a permission from "granted" to "prompt".

```ts
// deno run --unstable --allow-read=/foo main.ts

const desc = { name: "read", path: "/foo" };
console.log(await Deno.permissions.revoke(desc));
// PermissionStatus { state: "prompt" }
```

However, what happens when you try to revoke a permission which is _partial_ to
one granted on the CLI?

```ts
// deno run --unstable --allow-read=/foo main.ts

const desc = { name: "read", path: "/foo/bar" };
console.log(await Deno.permissions.revoke(desc));
// PermissionStatus { state: "granted" }
```

It was not revoked.

To understand this behaviour, imagine that Deno stores an internal set of
_explicitly granted permission descriptors_. Specifying `--allow-read=/foo,/bar`
on the CLI initializes this set to:

```ts
[
  { name: "read", path: "/foo" },
  { name: "read", path: "/bar" },
];
```

Granting a runtime request for `{ name: "write", path: "/foo" }` updates the set
to:

```ts
[
  { name: "read", path: "/foo" },
  { name: "read", path: "/bar" },
  { name: "write", path: "/foo" },
];
```

Deno's permission revocation algorithm works by removing every element from this
set which the argument permission descriptor is _stronger than_. So to ensure
`desc` is not longer granted, pass an argument descriptor _stronger than_
whichever _explicitly granted permission descriptor_ is _stronger than_ `desc`.

```ts
// deno run --unstable --allow-read=/foo main.ts

const desc = { name: "read", path: "/foo/bar" };
console.log(await Deno.permissions.revoke(desc)); // Insufficient.
// PermissionStatus { state: "granted" }

const strongDesc = { name: "read", path: "/foo" };
await Deno.permissions.revoke(strongDesc); // Good.

console.log(await Deno.permissions.query(desc));
// PermissionStatus { state: "prompt" }
```
