// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals, fail } from "@std/assert";
import dns, { lookupService } from "node:dns";
import dnsPromises, {
  lookup as lookupPromise,
  lookupService as lookupServicePromise,
} from "node:dns/promises";
import { ErrnoException } from "ext:deno_node/_global.d.ts";

interface LookupServiceResult {
  hostname: string;
  service: string;
}

const address = "8.8.8.8";
const port = 80;

Deno.test("lookupService with callback", async () => {
  // Named import
  const result = await new Promise<LookupServiceResult>(
    (resolve, reject) => {
      lookupService(address, port, (err, hostname, service) => {
        if (err) reject(err);
        resolve({ hostname, service });
      });
    },
  );
  assertEquals(typeof result.hostname, "string");
  assertEquals(typeof result.service, "string");

  // Default import
  const defaultImportResult = await new Promise<LookupServiceResult>(
    (resolve, reject) => {
      dns.lookupService(address, port, (err, hostname, service) => {
        if (err) reject(err);
        resolve({ hostname, service });
      });
    },
  );
  assertEquals(typeof defaultImportResult.hostname, "string");
  assertEquals(typeof defaultImportResult.service, "string");
});

Deno.test("lookupService promise", async () => {
  // Named import
  const result = await lookupServicePromise(address, port);
  assertEquals(typeof result.hostname, "string");
  assertEquals(typeof result.service, "string");

  // Default import
  const defaultImportResult = await dnsPromises.lookupService(
    address,
    port,
  );
  assertEquals(typeof defaultImportResult.hostname, "string");
  assertEquals(typeof defaultImportResult.service, "string");
});

Deno.test("lookupService not found", async () => {
  const address = "10.0.0.0";

  // Promise
  try {
    await lookupServicePromise(address, port);
    fail();
  } catch (err) {
    assertEquals(
      (err as ErrnoException).message,
      "getnameinfo ENOTFOUND 10.0.0.0",
    );
    assertEquals((err as ErrnoException).code, "ENOTFOUND");
    assertEquals((err as ErrnoException).syscall, "getnameinfo");
  }

  // Callback
  await new Promise<void>(
    (resolve, reject) => {
      dns.lookupService(address, port, (err) => {
        if (err) reject(err);
        resolve();
      });
    },
  ).then(() => fail(), (err) => {
    assertEquals(
      (err as ErrnoException).message,
      "getnameinfo ENOTFOUND 10.0.0.0",
    );
    assertEquals((err as ErrnoException).code, "ENOTFOUND");
    assertEquals((err as ErrnoException).syscall, "getnameinfo");
  });
});

Deno.test("[node/dns] lookup accepts string family values", async () => {
  const ipv4Result = await lookupPromise("localhost", { family: "IPv4" });
  assertEquals(ipv4Result.family, 4);

  const ipv6Result = await lookupPromise("localhost", { family: "IPv6" });
  assertEquals(ipv6Result.family, 6);
});
