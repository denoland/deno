// Request.signal must not abort while the response body is still streaming,
// nor after the request completes successfully.
let abortedDuringStream = false;
const abortedAfterCompletion = Promise.withResolvers<void>();

const server = Deno.serve({ port: 0, onListen: () => {} }, (req) => {
  let i = 0;
  const body = new ReadableStream({
    async pull(controller) {
      await new Promise((r) => setTimeout(r, 20));
      if (req.signal.aborted) {
        abortedDuringStream = true;
        controller.error(new Error("request signal aborted mid-stream"));
        return;
      }
      if (++i > 5) {
        controller.close();
        req.signal.addEventListener(
          "abort",
          () => abortedAfterCompletion.resolve(),
        );
        if (req.signal.aborted) abortedAfterCompletion.resolve();
        return;
      }
      controller.enqueue(new TextEncoder().encode(`chunk${i};`));
    },
  });
  return new Response(body);
});

const res = await fetch(`http://127.0.0.1:${server.addr.port}/`);
const text = await res.text();
console.log("body:", text);
console.log("aborted during stream:", abortedDuringStream);

// The signal must not abort on a successfully completed request.
let timer: number;
const result = await Promise.race([
  abortedAfterCompletion.promise.then(() => true),
  new Promise((r) => timer = setTimeout(r, 3000)).then(() => false),
]);
clearTimeout(timer!);
console.log("aborted after completion:", result);

await server.shutdown();
