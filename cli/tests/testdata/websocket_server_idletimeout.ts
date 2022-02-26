import { assertEquals } from "../../../test_util/std/testing/asserts.ts";
import { deferred } from "../../../test_util/std/async/deferred.ts";

const errorDeferred = deferred();
const closeDeferred = deferred();

const listener = Deno.listen({ port: 4509 });
console.log("READY");
const httpConn = Deno.serveHttp(await listener.accept());
const { request, respondWith } = (await httpConn.nextRequest())!;
const { response, socket } = Deno.upgradeWebSocket(request, {
  idleTimeout: 1,
});
socket.onerror = (e) => {
  assertEquals((e as ErrorEvent).message, "No response from ping frame.");
  errorDeferred.resolve();
};
socket.onclose = (e) => {
  assertEquals(e.reason, "No response from ping frame.");
  closeDeferred.resolve();
};
await respondWith(response);

await errorDeferred;
await closeDeferred;
listener.close();
