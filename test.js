const encoder = new TextEncoder();

const stream = new ReadableStream({
  start(controller) {
    let count = 0;
    const timerId = setInterval(() => {
      if (count == 10) {
        clearInterval(timerId);
        controller.close();
      } else {
        controller.enqueue(encoder.encode("Foo!"));
        count++;
      }
    }, 1000);
  },
});

// const req = await fetch("https://http2.golang.org/ECHO", {
//   method: "PUT",
//   body: stream,
// });

const decoder = new TextDecoder();

// for await (const chunk of req.body) {
//   const txt = decoder.decode(chunk);
//   console.log(txt);
// }

const channel = "1234567890";

fetch(`https://fetch-request-stream.glitch.me/send?channel=${channel}`, {
  method: "POST",
  headers: { "Content-Type": "text/plain" },
  body: stream,
}).then((res) => res.body.cancel());

fetch(`https://fetch-request-stream.glitch.me/receive?channel=${channel}`).then(
  async (res) => {
    const reader = res.body.getReader();
    while (true) {
      const { done, value } = await reader.read();
      if (done) return;
      console.log(decoder.decode(value));
    }
  },
);

setInterval(() => console.table(Deno.resources()), 500);
