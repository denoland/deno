## First steps

This page contains some examples to teach you about the fundamentals of Deno.

This document assumes that you have some prior knowledge of JavaScript,
especially about `async`/`await`. If you have no prior knowledge of JavaScript,
you might want to follow a guide
[on the basics of JavaScript](https://developer.mozilla.org/en-US/docs/Learn/JavaScript)
before attempting to start with Deno.

### Hello World

Deno is a runtime for JavaScript/TypeScript which tries to be web compatible and
use modern features wherever possible.

Browser compatibility means a `Hello World` program in Deno is the same as the
one you can run in the browser:

```ts
console.log("Welcome to Deno!");
```

Try the program:

```shell
deno run https://deno.land/std@$STD_VERSION/examples/welcome.ts
```

### Making an HTTP request

Many programs use HTTP requests to fetch data from a webserver. Let's write a
small program that fetches a file and prints its contents out to the terminal.

Just like in the browser you can use the web standard
[`fetch`](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API) API to
make HTTP calls:

```ts
const url = Deno.args[0];
const res = await fetch(url);

const body = new Uint8Array(await res.arrayBuffer());
await Deno.stdout.write(body);
```

Let's walk through what this application does:

1. We get the first argument passed to the application, and store it in the
   `url` constant.
2. We make a request to the url specified, await the response, and store it in
   the `res` constant.
3. We parse the response body as an
   [`ArrayBuffer`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/ArrayBuffer),
   await the response, and convert it into a
   [`Uint8Array`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Uint8Array)
   to store in the `body` constant.
4. We write the contents of the `body` constant to `stdout`.

Try it out:

```shell
deno run https://deno.land/std@$STD_VERSION/examples/curl.ts https://example.com
```

You will see this program returns an error regarding network access, so what did
we do wrong? You might remember from the introduction that Deno is a runtime
which is secure by default. This means you need to explicitly give programs the
permission to do certain 'privileged' actions, such as access the network.

Try it out again with the correct permission flag:

```shell
deno run --allow-net=example.com https://deno.land/std@$STD_VERSION/examples/curl.ts https://example.com
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
const filenames = Deno.args;
for (const filename of filenames) {
  const file = await Deno.open(filename);
  await Deno.copy(file, Deno.stdout);
  file.close();
}
```

The `copy()` function here actually makes no more than the necessary
kernel→userspace→kernel copies. That is, the same memory from which data is read
from the file, is written to stdout. This illustrates a general design goal for
I/O streams in Deno.

Try the program:

```shell
deno run --allow-read https://deno.land/std@$STD_VERSION/examples/cat.ts /etc/passwd
```

### TCP server

This is an example of a server which accepts connections on port 8080, and
returns to the client anything it sends.

```ts
const hostname = "0.0.0.0";
const port = 8080;
const listener = Deno.listen({ hostname, port });
console.log(`Listening on ${hostname}:${port}`);
for await (const conn of listener) {
  Deno.copy(conn, conn);
}
```

For security reasons, Deno does not allow programs to access the network without
explicit permission. To allow accessing the network, use a command-line flag:

```shell
deno run --allow-net https://deno.land/std@$STD_VERSION/examples/echo_server.ts
```

To test it, try sending data to it with netcat:

```shell
$ nc localhost 8080
hello world
hello world
```

Like the `cat.ts` example, the `copy()` function here also does not make
unnecessary memory copies. It receives a packet from the kernel and sends it
back, without further complexity.

### More examples

You can find more examples, like an HTTP file server, in the `Examples` chapter.
