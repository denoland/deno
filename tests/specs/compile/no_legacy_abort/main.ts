const aborted = Promise.withResolvers<boolean>();
const server = Deno.serve({ port: 0, onListen() {} }, (req, info) => {
  const { signal } = req;
  info.completed.then(() => aborted.resolve(signal.aborted));
  return new Response("ok");
});

const res = await fetch(`http://127.0.0.1:${server.addr.port}/`);
console.log(await res.text());
console.log("aborted:", await aborted.promise);
await server.shutdown();
