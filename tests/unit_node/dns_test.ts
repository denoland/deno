// Copyright 2018-2026 the Deno authors. MIT license.
import { assert, assertEquals, fail } from "@std/assert";
import dns, { getDefaultResultOrder, lookupService } from "node:dns";
import dnsPromises, {
  getDefaultResultOrder as getDefaultResultOrderPromise,
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

Deno.test("[node/dns] getDefaultResultOrder returns valid order", () => {
  // Named export from dns
  const order = getDefaultResultOrder();
  assertEquals(typeof order, "string");
  assert(
    ["ipv4first", "ipv6first", "verbatim"].includes(order),
    `unexpected order: ${order}`,
  );

  // Default export from dns
  assertEquals(dns.getDefaultResultOrder(), order);

  // dns/promises named export
  assertEquals(getDefaultResultOrderPromise(), order);

  // dns.promises
  assertEquals(dns.promises.getDefaultResultOrder(), order);

  // dnsPromises default export
  assertEquals(dnsPromises.getDefaultResultOrder(), order);
});

Deno.test("[node/dns] getDefaultResultOrder defaults to verbatim", () => {
  // Node.js defaults to "verbatim" since v17 (no IPv4-first reordering),
  // unless `--dns-result-order` is passed. Regression test for #34781.
  assertEquals(getDefaultResultOrder(), "verbatim");
});

Deno.test("[node/dns] getDefaultResultOrder reflects setDefaultResultOrder", () => {
  const original = dns.getDefaultResultOrder();
  try {
    dns.setDefaultResultOrder("ipv4first");
    assertEquals(dns.getDefaultResultOrder(), "ipv4first");

    dns.setDefaultResultOrder("verbatim");
    assertEquals(dns.getDefaultResultOrder(), "verbatim");
  } finally {
    // Restore original
    dns.setDefaultResultOrder(original);
  }
});

Deno.test("[node/dns] lookup accepts string family values", async () => {
  const ipv4Result = await lookupPromise("localhost", { family: "IPv4" });
  assertEquals(ipv4Result.family, 4);

  const ipv6Result = await lookupPromise("localhost", { family: "IPv6" });
  assertEquals(ipv6Result.family, 6);
});

// Regression test for https://github.com/denoland/deno/issues/25927
// `dns.lookup` must consult the operating system resolver (which reads the
// hosts file, e.g. /etc/hosts) rather than querying DNS servers directly.
// `localhost` is present in the hosts file on every supported platform and is
// not resolvable through public DNS, so it exercises that path.
Deno.test("[node/dns] lookup uses the system resolver / hosts file", async () => {
  const { address, family } = await new Promise<
    { address: string; family: number }
  >((resolve, reject) => {
    dns.lookup("localhost", (err, address, family) => {
      if (err) reject(err);
      else resolve({ address, family });
    });
  });
  assert(
    address === "127.0.0.1" || address === "::1",
    `unexpected address for localhost: ${address}`,
  );
  assert(family === 4 || family === 6, `unexpected family: ${family}`);

  const all = await lookupPromise("localhost", { all: true });
  assert(Array.isArray(all) && all.length > 0, "expected at least one address");
  assert(
    all.every(({ address }) => address === "127.0.0.1" || address === "::1"),
    `unexpected addresses for localhost: ${JSON.stringify(all)}`,
  );
});

// Regression test for https://github.com/denoland/deno/issues/34801
// `dns.lookup` must follow Node.js behavior when `hostname` is falsy
// (undefined / null / empty string): the callback is invoked with
// (null, null, family) instead of throwing synchronously.
Deno.test("[node/dns] lookup with falsy hostname invokes callback", async () => {
  for (const hostname of [undefined, null, ""]) {
    const result = await new Promise<
      { error: unknown; address: unknown; family: unknown }
    >((resolve) => {
      // deno-lint-ignore no-explicit-any
      dns.lookup(hostname as any, (error, address, family) => {
        resolve({ error, address, family });
      });
    });
    assertEquals(result, { error: null, address: null, family: 4 });
  }

  // family argument is honored when 6.
  const result6 = await new Promise<
    { error: unknown; address: unknown; family: unknown }
  >((resolve) => {
    dns.lookup(
      // deno-lint-ignore no-explicit-any
      undefined as any,
      6,
      (error, address, family) => resolve({ error, address, family }),
    );
  });
  assertEquals(result6, { error: null, address: null, family: 6 });

  // options.family = 6
  const resultOpt6 = await new Promise<
    { error: unknown; address: unknown; family: unknown }
  >((resolve) => {
    dns.lookup(
      // deno-lint-ignore no-explicit-any
      undefined as any,
      { family: 6 },
      (error, address, family) => resolve({ error, address, family }),
    );
  });
  assertEquals(resultOpt6, { error: null, address: null, family: 6 });

  // options.all = true returns an empty array via the callback.
  const resultAll = await new Promise<
    { error: unknown; addresses: unknown }
  >((resolve) => {
    dns.lookup(
      // deno-lint-ignore no-explicit-any
      undefined as any,
      { all: true },
      // deno-lint-ignore no-explicit-any
      (error, addresses: any) => resolve({ error, addresses }),
    );
  });
  assertEquals(resultAll, { error: null, addresses: [] });
});

Deno.test("[node/dns] promises.lookup with falsy hostname resolves", async () => {
  for (const hostname of [undefined, null, ""]) {
    // deno-lint-ignore no-explicit-any
    const result = await lookupPromise(hostname as any);
    assertEquals(result as unknown, { address: null, family: 4 });
  }

  // deno-lint-ignore no-explicit-any
  const result6 = await lookupPromise(undefined as any, { family: 6 });
  assertEquals(result6 as unknown, { address: null, family: 6 });

  // deno-lint-ignore no-explicit-any
  const resultAll = await lookupPromise(undefined as any, { all: true });
  assertEquals(resultAll, []);
});

// Regression test for https://github.com/denoland/deno/issues/25927
// A failed `dns.lookup` must report the real libuv error code and errno
// (ENOTFOUND / -3008) like Node.js, instead of flattening every failure to
// EAI_NODATA (-3007).
Deno.test("[node/dns] lookup of a missing host reports ENOTFOUND", async () => {
  const err = await new Promise<ErrnoException>((resolve) => {
    dns.lookup("nonexistent-host.invalid", (err) => {
      resolve(err as unknown as ErrnoException);
    });
  });
  assert(err, "expected an error for an unresolvable host");
  assertEquals(err.code, "ENOTFOUND");
  assertEquals(err.errno, -3008);
  assertEquals(err.syscall, "getaddrinfo");
  assertEquals(err.hostname, "nonexistent-host.invalid");
});
