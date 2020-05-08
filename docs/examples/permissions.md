## Inspecting and revoking permissions

> This program makes use of an unstable Deno feature. Learn more about
> [unstable features](../../runtime/unstable).

Sometimes a program may want to revoke previously granted permissions. When a
program, at a later stage, needs those permissions, it will fail.

```ts
// lookup a permission
const status = await Deno.permissions.query({ name: "write" });
if (status.state !== "granted") {
  throw new Error("need write permission");
}

const log = await Deno.open("request.log", "a+");

// revoke some permissions
await Deno.permissions.revoke({ name: "read" });
await Deno.permissions.revoke({ name: "write" });

// use the log file
const encoder = new TextEncoder();
await log.write(encoder.encode("hello\n"));

// this will fail.
await Deno.remove("request.log");
```
