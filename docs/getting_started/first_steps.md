## First steps

This page contains some simple examples that can teach you about the
fundamentals of Deno.

This document assumes that you have some prior knowledge of JavaScript,
especially about `async`/`await`. If you have no prior knowledge of JavaScript,
you might want to folow a guide
[on the basics of JavaScript](https://developer.mozilla.org/en-US/docs/Learn/JavaScript)
before attempting to start with Deno.

### Hello World

Deno is a runtime for JavaScript and TypeScript and tries to be web compatible
and use modern features whereever possible.

Because of this browser compatibility a simple `Hello World` program is actually
no different to one you can run in the browser:

```typescript
console.log("Welcome to Deno 🦕");
```

Try the program:

```bash
deno run https://deno.land/std/examples/welcome.ts
```

### Making an HTTP request

Something a lot of programs do is fetching data from from a webserver via an
HTTP request. Lets write a small program that fetches a file and prints the
content to the terminal.

Just like in the browser you can use the web standard
[`fetch`](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API) API to
make HTTP calls:

```typescript
const url = Deno.args[0];
const res = await fetch(url);

const body = new Uint8Array(await res.arrayBuffer());
await Deno.stdout.write(body);
```

Lets walk through what this application does:

1. We get the first argument passed to the application and store it in the
   variable `url`.
2. We make a request to the url specified, await the response, and store it in a
   variable named `res`.
3. We parse the response body as an
   [`ArrayBuffer`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/ArrayBuffer),
   await the response, convert it into a
   [`Uint8Array`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Uint8Array)
   and store it in the variable `body`.
4. We write the contents of the `body` variable to `stdout`.

Try it out:

```bash
deno run https://deno.land/std/examples/curl.ts https://example.com
```

You will see that this program returns an error regarding network access, so
what did we do wrong? You might remember from the introduction that Deno is a
runtime that is secure by default. This means that you need to explicitly give
programs the permission to do certain 'privledged' actions like network access.

Try it out again with the correct permission flag:

```bash
deno run --allow-net=example.com https://deno.land/std/examples/curl.ts https://example.com
```

### Reading a file

Deno also provides APIs which do not come from the web. These are all contained
in the `Deno` global. You can find documentation for these APIs on
[doc.deno.land](https://doc.deno.land/https/github.com/denoland/deno/releases/latest/download/lib.deno.d.ts).

Filesystem APIs for example do not have a web standard form, so Deno provides
its own API.

In this program each command-line argument is assumed to be a filename, the file
is opened, and printed to stdout.

```ts
for (let i = 0; i < Deno.args.length; i++) {
  let filename = Deno.args[i];
  let file = await Deno.open(filename);
  await Deno.copy(file, Deno.stdout);
  file.close();
}
```

The `copy()` function here actually makes no more than the necessary kernel ->
userspace -> kernel copies. That is, the same memory from which data is read
from the file, is written to stdout. This illustrates a general design goal for
I/O streams in Deno.

Try the program:

```bash
deno run --allow-read https://deno.land/std/examples/cat.ts /etc/passwd
```

### A simple TCP server

This is an example of a simple server which accepts connections on port 8080,
and returns to the client anything it sends.

```ts
const listener = Deno.listen({ port: 8080 });
console.log("listening on 0.0.0.0:8080");
for await (const conn of listener) {
  Deno.copy(conn, conn);
}
```

For security reasons, Deno does not allow programs to access the network without
explicit permission. To allow accessing the network, use a command-line flag:

```shell
$ deno run --allow-net https://deno.land/std/examples/echo_server.ts
```

To test it, try sending data to it with netcat:

```shell
$ nc localhost 8080
hello world
hello world
```

Like the `cat.ts` example, the `copy()` function here also does not make
unnecessary memory copies. It receives a packet from the kernel and sends back,
without further complexity.

### More examples

You can find more examples, like an HTTP file server, in the `Examples` chapter.
