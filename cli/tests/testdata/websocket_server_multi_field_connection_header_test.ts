import { deferred } from "../unit/test_util.ts";

const promise = deferred();
const listener = Deno.listen({ port: 4319 });
console.log("READY");
const conn = await listener.accept();
const httpConn = Deno.serveHttp(conn);
const { request, respondWith } = (await httpConn.nextRequest())!;
const {
  response,
  socket,
} = Deno.upgradeWebSocket(request);
socket.onerror = () => Deno.exit(1);
socket.onopen = () => socket.close();
socket.onclose = () => promise.resolve();
await respondWith(response);
await promise;
