// deno-fmt-ignore-file
import { toFileUrl } from "@std/path/to-file-url";

function tryGetCwd() {
  // will throw in one test but not the other
  try {
    return Deno.cwd()
  } catch {
    return import.meta.dirname;
  }
}

const fooExePath = tryGetCwd() + "/foo" + (Deno.build.os === "windows" ? ".exe" : "");
postMessage({
  envGlobal: (await Deno.permissions.query({ name: "env" })).state,
  envFoo: (await Deno.permissions.query({ name: "env", variable: "foo" })).state,
  envAbsent: (await Deno.permissions.query({ name: "env", variable: "absent" })).state,
  netGlobal: (await Deno.permissions.query({ name: "net" })).state,
  netFoo: (await Deno.permissions.query({ name: "net", host: "foo" })).state,
  netFoo8000: (await Deno.permissions.query({ name: "net", host: "foo:8000" })).state,
  netBar: (await Deno.permissions.query({ name: "net", host: "bar" })).state,
  netBar8000: (await Deno.permissions.query({ name: "net", host: "bar:8000" })).state,
  ffiGlobal: (await Deno.permissions.query({ name: "ffi" })).state,
  ffiFoo: (await Deno.permissions.query({ name: "ffi", path: new URL("foo", import.meta.url) })).state,
  ffiBar: (await Deno.permissions.query({ name: "ffi", path: "bar" })).state,
  ffiAbsent: (await Deno.permissions.query({ name: "ffi", path: "absent" })).state,
  readGlobal: (await Deno.permissions.query({ name: "read" })).state,
  readFoo: (await Deno.permissions.query({ name: "read", path: new URL("foo", import.meta.url) })).state,
  readBar: (await Deno.permissions.query({ name: "read", path: "bar" })).state,
  readAbsent: (await Deno.permissions.query({ name: "read", path: "../absent" })).state,
  runGlobal: (await Deno.permissions.query({ name: "run" })).state,
  runFoo: (await Deno.permissions.query({ name: "run", command: toFileUrl(fooExePath) })).state,
  runFooPath: (await Deno.permissions.query({ name: "run", command: fooExePath })).state,
  runBar: (await Deno.permissions.query({ name: "run", command: "bar" })).state,
  runBaz: (await Deno.permissions.query({ name: "run", command: "./baz" })).state,
  runUnresolved: (await Deno.permissions.query({ name: "run", command: "unresolved-exec" })).state,
  runAbsent: (await Deno.permissions.query({ name: "run", command: "absent" })).state,
  writeGlobal: (await Deno.permissions.query({ name: "write" })).state,
  writeFoo: (await Deno.permissions.query({ name: "write", path: new URL("foo", import.meta.url) })).state,
  writeBar: (await Deno.permissions.query({ name: "write", path: "bar" })).state,
  writeAbsent: (await Deno.permissions.query({ name: "write", path: "absent" })).state,
});
