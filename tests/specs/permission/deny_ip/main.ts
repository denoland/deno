import * as http from "node:http";
import * as https from "node:https";

// deno-lint-ignore no-explicit-any
async function printResult<F extends () => Promise<any>>(
  name: string,
  fn: F,
) {
  const prom = Promise.withResolvers<void>();
  let threw = false;
  try {
    await fn();
    prom.resolve();
  } catch (error) {
    console.error(`${name} threw error: ${error}`);
    return;
  }
  await prom.promise;
  if (!threw) {
    console.error(`${name}: did not throw`);
  }
}

async function wrapEvent<F extends (ev: any) => any>(
  fn: F,
) {
  return new Promise<void>((resolve, reject) => {
    const errListener = (error: unknown) => {
      reject(error);
    };
    process.once("uncaughtException", errListener);
    process.once("unhandledRejection", errListener);
    fn((_) => {
      process.removeListener("uncaughtException", errListener);
      process.removeListener("unhandledRejection", errListener);
      resolve();
    });
  });
}

await printResult("fetch", () => fetch("http://localhost:4545"));
await printResult(
  "http.request",
  () =>
    wrapEvent(
      (listener) => http.request("http://localhost:4545", listener),
    ),
);
await printResult(
  "https.request",
  () =>
    wrapEvent(
      (listener) => https.request("https://localhost:4545", listener),
    ),
);
await printResult(
  "Deno.listen",
  () => Deno.listen({ hostname: "localhost", port: "3000" }),
);

await printResult(
  "Deno.listenTls",
  async () =>
    Deno.listenTls({
      hostname: "localhost",
      port: 3000,
      key: await Deno.readTextFile("../../../testdata/tls/localhost.key"),
      cert: await Deno.readTextFile("../../../testdata/tls/localhost.crt"),
    }),
);

await printResult(
  "Deno.connectTls",
  () => Deno.connectTls({ hostname: "localhost", port: 3000 }),
);

await printResult(
  "Deno.listenDatagram",
  () =>
    Deno.listenDatagram({
      hostname: "localhost",
      port: 3000,
      transport: "udp",
    }),
);

await printResult(
  "Deno.connect tcp",
  () => Deno.connect({ hostname: "localhost", port: 3000, transport: "tcp" }),
);
