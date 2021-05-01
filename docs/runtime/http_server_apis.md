## HTTP Server APIs

As of Deno 1.9 and later, _native_ HTTP server APIs were introduced which allow
users to create robust and performant web servers in Deno.

The API tries to leverage as much of the web standards as is possible as well as
tries to be simple and straight forward.

> ℹ️ The APIs are currently unstable, meaning they can change in the future in
> breaking ways and should be carefully considered before using in production
> code. They require the `--unstable` flag to make them available.

### Listening for a connection

In order to accept requests, first you need to listen for a connection on a
network port. To do this in Deno, you use `Deno.listen()`:

```ts
const server = Deno.listen({ port: 8080 });
```

> ℹ️ When supplying a port, Deno assumes you are going to listen on a TCP socket
> as well as bind to the localhost. You can specify `transport: "tcp"` to be
> more explicit as well as provide an IP address or hostname in the `hostname`
> property as well.

If there is an issue with opening the network port, `Deno.listen()` will throw,
so often in a server sense, you will want to wrap it in the `try ... catch`
block in order to handle exceptions, like the port already being in use.

You can also listen for a TLS connection (e.g. HTTPS) using `Deno.listenTls()`:

```ts
const server = Deno.listenTls({
  port: 8443,
  certFile: "localhost.crt",
  keyFile: "localhost.key",
  alpnProtocols: ["h2", "http/1.1"],
});
```

The `certFile` and `keyFile` options are required and point to the appropriate
certificate and key files for the server. They are relative to the CWD for Deno.
The `alpnProtocols` property is optional, but if you want to be able to support
HTTP/2 on the server, you add the protocols here, as the protocol negotiation
happens during the TLS negotiation with the client and server.

> ℹ️ Generating SSL certificates is outside of the scope of this documentation.
> There are many resources on the web which address this.

### Handling connections

Once we are listening for a connection, we need to handle the connection. The
return value of `Deno.listen()` or `Deno.listenTls()` is a `Deno.Listener` which
is an async iterable which yields up `Deno.Conn` connections as well as provide
a couple methods for handling connections.

To use it as an async iterable we would do something like this:

```ts
const server = Deno.listen({ port: 8080 });

for await (const conn of server) {
  // ...handle the connection...
}
```

Every connection made would yielded up a `Deno.Conn` assigned to `conn`. Then
further processing can be applied to the connection.

There is also the `.accept()` method on the listener which can be used:

```ts
const server = Deno.listen({ port: 8080 });

while (true) {
  const conn = server.accept();
  if (conn) {
    // ... handle the connection ...
  } else {
    // The listener has closed
    break;
  }
}
```

Whether using the async iterator or the `.accept()` method, exceptions can be
thrown and robust production code should handle these using `try ... catch`
blocks. Especially when it comes to accepting TLS connections, there can be many
conditions, like invalid or unknown certificates which can be surfaced on the
listener and might need handling in the user code.

A listener also has a `.close()` method which can be used to close the listener.

### Serving HTTP

Once a connection is accepted, you can use `Deno.serveHttp()` to handle HTTP
requests and responses on the connection. `Deno.serveHttp()` returns a
`Deno.HttpConn`. A `Deno.HttpConn` is like a `Deno.Listener` in that requests
the connection receives from the client are asynchronously yielded up as a
`Deno.RequestEvent`.

To deal with HTTP requests as async iterable it would look something like this:

```ts
const server = Deno.listen({ port: 8080 });

for await (const conn of server) {
  (async () => {
    const httpConn = Deno.serveHttp(conn);
    for await (const requestEvent of httpConn) {
      // ... handle requestEvent ...
    }
  })();
}
```

The `Deno.HttpConn` also has the method `.nextRequest()` which can be used to
await the next request. It would look something like this:

```ts
const server = Deno.listen({ port: 8080 });

while (true) {
  const conn = server.accept();
  if (conn) {
    (async () => {
      const httpConn = Deno.serveHttp(conn);
      while (true) {
        const requestEvent = await httpConn.nextRequest();
        if (requestEvent) {
          // ... handle requestEvent ...
        } else {
          // the connection has finished
          break;
        }
      }
    })();
  } else {
    // The listener has closed
    break;
  }
}
```

Note that in both cases we are using an IIFE to create an inner function to deal
with each connection. If we awaited the HTTP requests in the same function scope
as the one we were receiving the connections, we would be blocking accepting
additional connections, which would make it seem that our server was "frozen".
In practice, it might make more sense to have a separate function all together:

```ts
async function handle(conn: Deno.Conn) {
  const httpConn = Deno.serveHttp(conn);
  for await (const requestEvent of httpConn) {
    // ... handle requestEvent
  }
}

const server = Deno.listen({ port: 8080 });

for await (const conn of server) {
  handle(conn);
}
```

In the examples from this point on, we will focus on what would occur within an
example `handle()` function and remove the listening and connection
"boilerplate".

### HTTP Requests and Responses

HTTP requests and responses in Deno are essentially the inverse of web standard
[Fetch API](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API). The
Deno HTTP Server API and the Fetch API leverage the
[`Request`](https://developer.mozilla.org/en-US/docs/Web/API/Request) and
[`Response`](https://developer.mozilla.org/en-US/docs/Web/API/Response) object
classes. So if you are familiar with the Fetch API you just need to flip them
around in your mind and now it is a server API.

As mentioned above, a `Deno.HttpConn` asynchronously yields up
`Deno.RequestEvent`s. These request events contain a `.request` property and a
`.respondWith()` method.

The `.request` property is an instance of the `Request` class with the
information about the request. For example, if we wanted to know what URL path
was being requested, we would do something like this:

```ts
async function handle(conn: Deno.Conn) {
  const httpConn = Deno.serveHttp(conn);
  for await (const requestEvent of httpConn) {
    const url = new URL(requestEvent.request.url);
    console.log(`path: ${url.path}`);
  }
}
```

The `.respondWith()` method is how we complete a request. The method takes
either a `Response` object or a `Promise` which resolves with a `Response`
object. Responding with a basic "hello world" would look like this:

```ts
async function handle(conn: Deno.Conn) {
  const httpConn = Deno.serveHttp(conn);
  for await (const requestEvent of httpConn) {
    await requestEvent.respondWith(new Response("hello world"), {
      status: 200,
    });
  }
}
```

Note that we awaited the `.respondWith()` method. It isn't required, but in
practice any errors in processing the response will cause the promise returned
from the method to be rejected, like if the client disconnected before all the
response could be sent. While there may not be anything your application needs
to do, not handling the rejection will cause an "unhandled rejection" to occur
which will terminate the Deno process, which isn't so good for a server. In
addition, you might want to await the promise returned in order to determine
when to do any cleanup from for the request/response cycle.

The web standard `Response` object is pretty powerful, allowing easy creation of
complex and rich responses to a client, and Deno strives to provide a `Response`
object that as closely matches the web standard as possible, so if you are
wondering how to send a particular response, checkout out the documentation for
the web standard
[`Response`](https://developer.mozilla.org/en-US/docs/Web/API/Response).

### HTTP/2 Support

HTTP/2 support is effectively transparent within the Deno runtime. Typically
HTTP/2 is negotiated between a client and a server during the TLS connection
setup via
[ALPN](https://en.wikipedia.org/wiki/Application-Layer_Protocol_Negotiation). To
enable this, you need to provide the protocols you want to support when you
start listening via the `alpnProtocols` property. This will enable the
negotiation to occur when the connection is made. For example:

```ts
const server = Deno.listenTls({
  port: 8443,
  certFile: "localhost.crt",
  keyFile: "localhost.key",
  alpnProtocols: ["h2", "http/1.1"],
});
```

The protocols are provided in order of preference. In practice, the only two
protocols that are supported currently are HTTP/2 and HTTP/1.1 which are
expressed as `h2` and `http/1.1`.

Currently Deno does not support upgrading a plain-text HTTP/1.1 connection to an
HTTP/2 cleartext connection via the `Upgrade` header (see:
[#10275](https://github.com/denoland/deno/issues/10275)), so therefore HTTP/2
support is only available via a TLS/HTTPS connection.
