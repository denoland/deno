import { getDenoShim, unstable } from "./deno_shim.ts";
import { assert, assertEquals } from "../testing/asserts.ts";

const { test } = Deno;

const denoShim = await getDenoShim();

test({
  name: "denoShim equality",
  fn() {
    assert(denoShim !== Deno);
  },
});

const unstableNames = [
  "umask",
  "linkSync",
  "link",
  "symlinkSync",
  "symlink",
  "dir",
  "loadavg",
  "osRelease",
  "openPlugin",
  "DiagnosticCategory",
  "formatDiagnostics",
  "transpileOnly",
  "compile",
  "bundle",
  "applySourceMap",
  "Signal",
  "SignalStream",
  "signal",
  "signals",
  "setRaw",
  "utimeSync",
  "utime",
  "ShutdownMode",
  "shutdown",
  "listenDatagram",
  "startTls",
  "kill",
  "Permissions",
  "permissions",
  "PermissionStatus",
  "hostname",
];

test({
  name: "deno_shim - property match",
  fn() {
    for (const key of Object.keys(Deno)) {
      if (unstableNames.includes(key)) {
        // denoShim does not contain unstable APIs.
        continue;
      }
      assert(key in denoShim, `${key} should be in denoShim`);
      assert(
        typeof denoShim[key as keyof typeof denoShim] ===
          typeof Deno[key as keyof typeof Deno],
        `Types of property ${key} should match.`
      );
      const desc = Object.getOwnPropertyDescriptor(denoShim, key);
      assert(desc);
      assertEquals(desc.enumerable, true, "should be enumerable");
      assertEquals(desc.writable, false, "should be read only");
      assertEquals(desc.configurable, false, "should not be configurable");
    }
  },
});

test({
  name: "deno_shim - unstable property match",
  async fn() {
    await unstable();
    const unstableShim = await getDenoShim();
    for (const key of unstableNames) {
      if (!(key in Deno)) {
        continue;
      }
      assert(key in unstableShim, `${key} should be in shim`);
      assert(
        typeof unstableShim[key as keyof typeof unstableShim] ===
          typeof Deno[key as keyof typeof Deno],
        `Types of property ${key} should match.`
      );
      const desc = Object.getOwnPropertyDescriptor(unstableShim, key);
      assert(desc);
      assertEquals(desc.enumerable, true, "should be enumerable");
      assertEquals(desc.writable, false, "should be read only");
      assertEquals(desc.configurable, false, "should not be configurable");
    }
  },
});
