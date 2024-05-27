function handler(_req: Request): Response {
  let timer: number | undefined = undefined;
  const body = new ReadableStream({
    start(controller) {
      timer = setInterval(() => {
        const message = `It is ${new Date().toISOString()}\n`;
        controller.enqueue(new TextEncoder().encode(message));
      }, 1000);
    },
    cancel() {
      if (timer !== undefined) {
        clearInterval(timer);
      }
    }
  });

  return new Response(body, {
    headers: {
      "content-type": "text/plain",
      "x-content-type-options": "nosniff"
    }
  });
}

Deno.serve(handler);
