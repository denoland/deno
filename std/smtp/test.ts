// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Based on https://github.com/golang/go/blob/92c732e901a732855f4b813e6676264421eceae9/src/net/smtp/smtp_test.go
// Copyright 2010 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

import {
  assert,
  assertEquals,
  assertStrictEquals,
  assertThrowsAsync,
} from "../testing/asserts.ts";
import { decode, encode } from "../encoding/utf8.ts";
import { deferred } from "../async/deferred.ts";
import type { Auth, AuthMessage, ServerInfo, SMTPClient } from "./mod.ts";
import { cramMD5Auth, createSMTPClient, plainAuth, sendMail } from "./mod.ts";
import { SMTPClientImpl } from "./_client.ts";
import { StringReader } from "../io/readers.ts";
import { BufReader, BufWriter } from "../io/bufio.ts";
import { TextProtoConn } from "../textproto/conn.ts";

async function assertFailsAsync(
  fn: () => Promise<void>,
  msg?: string,
): Promise<void> {
  await assertThrowsAsync(fn, Error, "", msg);
}

Deno.test({
  ignore: true,
  name: "[smtp] auth",
  fn: () => {
    type AuthTest = {
      auth: Auth;
      challenges: string[];
      name: string;
      responses: (string | null)[];
    };
    const authTests: AuthTest[] = [
      {
        auth: plainAuth(
          {
            identity: "",
            username: "user",
            password: "pass",
            host: "testserver",
          },
        ),
        challenges: [],
        name: "PLAIN",
        responses: ["\x00user\x00pass"],
      },
      {
        auth: plainAuth({
          identity: "foo",
          username: "bar",
          password: "baz",
          host: "testserver",
        }),
        challenges: [],
        name: "PLAIN",
        responses: ["foo\x00bar\x00baz"],
      },
      {
        auth: cramMD5Auth("user", "pass"),
        challenges: ["<123456.1322876914@testserver>"],
        name: "CRAM-MD5",
        responses: [null, "user 287eb355114cf5c471c26a875f1ca4ae"],
      },
    ];
    for (const test of authTests) {
      const message = test.auth.start(
        { name: "testserver", tls: true, auth: null },
      );
      assertStrictEquals(message.protocol, test.name);
      assertEquals(message.toServer, test.responses[0]);
      for (let i = 0; i < test.challenges.length; i++) {
        const challenge = test.challenges[i];
        const expected = test.responses[i + 1];
        const resp = test.auth.next(challenge, true);
        assertStrictEquals(resp, expected);
      }
    }
  },
});

Deno.test("[smtp] authPlain", () => {
  const tests: Array<{
    authName: string;
    server: ServerInfo;
    err?: Error;
  }> = [
    {
      authName: "servername",
      server: { name: "servername", tls: true, auth: null },
    },
    {
      // OK to use PlainAuth on localhost without TLS
      authName: "localhost",
      server: { name: "localhost", tls: false, auth: null },
    },
    {
      // NOT OK on non-localhost, even if server says PLAIN is OK.
      // (We don't know that the server is the real server.)
      authName: "servername",
      server: { name: "servername", auth: ["PLAIN"] },
      err: new Error("unencrypted connection"),
    },
    {
      authName: "servername",
      server: { name: "servername", auth: ["CRAM-MD5"] },
      err: new Error("unencrypted connection"),
    },
    {
      authName: "servername",
      server: { name: "attacker", tls: true, auth: null },
      err: new Error("wrong host name"),
    },
  ];

  for (const tt of tests) {
    const auth = plainAuth({
      identity: "foo",
      username: "bar",
      password: "baz",
      host: tt.authName,
    });
    let got: Error | undefined = undefined;
    try {
      auth.start(tt.server);
    } catch (err) {
      got = err;
    }
    assertEquals(got, tt.err);
  }
});

function noop(): void {}

function createFakeConn(r: Deno.Reader, w: Deno.Writer): Deno.Conn {
  return {
    rid: 100,
    localAddr: {
      transport: "tcp",
      hostname: "localhost",
      port: 25,
    },
    remoteAddr: {
      transport: "tcp",
      hostname: "localhost",
      port: 25,
    },
    read(p) {
      return r.read(p);
    },
    write(p) {
      return w.write(p);
    },
    close: noop,
    closeWrite: noop,
  };
}

function createFakeSMTPClient(
  r: Deno.Reader,
  w: Deno.Writer,
  host: string,
): SMTPClientImpl {
  const conn = createFakeConn(r, w);
  const c = new SMTPClientImpl(
    conn,
    new TextProtoConn(conn),
    host,
    "localhost",
    false,
  );
  return c;
}

// golang/go/issues/17794: don't send a trailing space on AUTH command when there's no password.
Deno.test("[smtp] clientAuthTrimSpace", async () => {
  const server = "220 hello world\r\n" +
    "200 some more";
  const wrote = new Deno.Buffer();
  const c = createFakeSMTPClient(
    new StringReader(server),
    wrote,
    "fake.host",
  );
  c._isTLS = true;
  c._didHello = true;
  await assertFailsAsync(() => c.auth(new ToServerEmptyAuth()));
  c.close();
  const got = decode(await Deno.readAll(wrote));
  const want = "AUTH FOOAUTH\r\n*\r\nQUIT\r\n";
  assertStrictEquals(got, want);
});

// `ToServerEmptyAuth` is an implementation of `Auth` that only implements
// the `start` method, and returns "FOOAUTH", nil, nil. Notably, it returns
// zero bytes for "toServer" so we can test that we don't send spaces at
// the end of the line. See TestClientAuthTrimSpace.
class ToServerEmptyAuth implements Auth {
  start(_server: ServerInfo) {
    return { protocol: "FOOAUTH", toServer: null };
  }

  next(_fromServer: string, _more: boolean): null {
    throw new Error("unexpected call");
  }
}

Deno.test("[smtp] basic", async () => {
  const server = `250 mx.google.com at your service
502 Unrecognized command.
250-mx.google.com at your service
250-SIZE 35651584
250-AUTH LOGIN PLAIN
250 8BITMIME
530 Authentication required
252 Send some mail, I'll try my best
250 User is valid
235 Accepted
250 Sender OK
250 Receiver OK
354 Go ahead
250 Data OK
221 OK
`.split("\n").join("\r\n");

  const client = `HELO localhost
EHLO localhost
EHLO localhost
MAIL FROM:<user@gmail.com> BODY=8BITMIME
VRFY user1@gmail.com
VRFY user2@gmail.com
AUTH PLAIN AHVzZXIAcGFzcw==
MAIL FROM:<user@gmail.com> BODY=8BITMIME
RCPT TO:<golang-nuts@googlegroups.com>
DATA
From: user@gmail.com
To: golang-nuts@googlegroups.com
Subject: Hooray for Go
Line 1
..Leading dot line .
Goodbye.
.
QUIT
`.split("\n").join("\r\n");

  const cmdbuf = new Deno.Buffer();
  const bcmdbuf = BufWriter.create(cmdbuf);
  const fakeConn = createFakeConn(
    BufReader.create(new StringReader(server)),
    bcmdbuf,
  );
  const c = new SMTPClientImpl(
    fakeConn,
    new TextProtoConn(fakeConn),
    "",
    "localhost",
  );
  await c._helo();
  await assertFailsAsync(
    () => c._ehlo(),
    "Expected first EHLO to fail",
  );
  await c._ehlo();

  c._didHello = true;
  const auth = await c.extension("aUtH");
  assertStrictEquals(auth, "LOGIN PLAIN");
  assertStrictEquals(await c.extension("DSN"), null, "Shouldn't support DSN");
  await assertFailsAsync(
    () => c.mail("user@gmail.com"),
    "MAIL should require authentication",
  );
  await assertFailsAsync(
    () => c.verify("user1@gmail.com"),
    "First VRFY: expected no verification",
  );
  await assertFailsAsync(
    () =>
      c.verify(
        "user2@gmail.com>\r\nDATA\r\nAnother injected message body\r\n.\r\nQUIT\r\n",
      ),
    "VRFY should have failed due to a message injection attempt",
  );
  await c.verify("user2@gmail.com");

  // fake TLS so authentication won't complain
  c._isTLS = true;
  c._serverName = "smtp.google.com";
  await c.auth(plainAuth({
    identity: "",
    username: "user",
    password: "pass",
    host: "smtp.google.com",
  }));

  await assertFailsAsync(
    () =>
      c.rcpt(
        "golang-nuts@googlegroups.com>\r\nDATA\r\nInjected message body\r\n.\r\nQUIT\r\n",
      ),
    "RCPT should have failed due to a message injection attempt",
  );
  await assertFailsAsync(
    () =>
      c.mail(
        "user@gmail.com>\r\nDATA\r\nAnother injected message body\r\n.\r\nQUIT\r\n",
      ),
    "MAIL should have failed due to a message injection attempt",
  );
  await c.mail("user@gmail.com");
  await c.rcpt("golang-nuts@googlegroups.com");
  const msg = `From: user@gmail.com
To: golang-nuts@googlegroups.com
Subject: Hooray for Go
Line 1
.Leading dot line .
Goodbye.`;
  const w = await c.data();
  await w.write(encode(msg));
  await w.close();
  await c.quit();

  await bcmdbuf.flush();
  const actualCmds = new Uint8Array(cmdbuf.capacity);
  await cmdbuf.read(actualCmds);

  assertStrictEquals(decode(actualCmds), client);
});

function testExtensions(
  name: string,
  fn: (
    c: SMTPClientImpl,
    bcmdbuf: BufWriter,
    cmdbuf: Deno.Buffer,
  ) => Promise<void>,
  server: string,
): void {
  Deno.test(`[smtp] extensions: ${name}`, () => {
    server = server.split("\n").join("\r\n");

    const cmdbuf = new Deno.Buffer();
    const bcmdbuf = BufWriter.create(cmdbuf);
    const fakeConn = createFakeConn(
      BufReader.create(new StringReader(server)),
      bcmdbuf,
    );
    const c = createFakeSMTPClient(
      BufReader.create(new StringReader(server)),
      bcmdbuf,
      "localhost",
    );

    return fn(c, bcmdbuf, cmdbuf);
  });
}

testExtensions(
  "helo",
  async (c, bcmdbuf, cmdbuf) => {
    const basicClient = `HELO localhost
MAIL FROM:<user@gmail.com>
QUIT
`;
    await c._helo();
    c._didHello = true;
    await c.mail("user@gmail.com");
    await c.quit();
    await bcmdbuf.flush();
    const actualCmds = await Deno.readAll(cmdbuf);
    const client = basicClient.split("\n").join("\r\n");
    assertStrictEquals(decode(actualCmds), client);
  },
  `250 mx.google.com at your service
250 Sender OK
221 Goodbye
`,
);

testExtensions(
  "ehlo",
  async (c, bcmdbuf, cmdbuf) => {
    const basicClient = `EHLO localhost
MAIL FROM:<user@gmail.com>
QUIT
`;
    await c.hello("localhost");
    assertStrictEquals(
      await c.extension("8BITMIME"),
      null,
      "Shouldn't support 8BITMIME",
    );
    assertStrictEquals(
      await c.extension("SMTPUTF8"),
      null,
      "Shouldn't support SMTPUTF8",
    );
    await c.mail("user@gmail.com");
    await c.quit();

    await bcmdbuf.flush();
    const actualCmds = await Deno.readAll(cmdbuf);
    const client = basicClient.split("\n").join("\r\n");
    assertStrictEquals(decode(actualCmds), client);
  },
  `250-mx.google.com at your service
250 SIZE 35651584
250 Sender OK
221 Goodbye
`,
);

testExtensions(
  "ehlo 8bitmime",
  async (c, bcmdbuf, cmdbuf) => {
    const basicClient = `EHLO localhost
MAIL FROM:<user@gmail.com> BODY=8BITMIME
QUIT
`;
    await c.hello("localhost");
    assert(await c.extension("8BITMIME") != null, "Should support 8BITMIME");
    assertStrictEquals(await c.extension("SMTPUTF8"), null);
    await c.mail("user@gmail.com");
    await c.quit();

    await bcmdbuf.flush();
    const actualCmds = await Deno.readAll(cmdbuf);
    const client = basicClient.split("\n").join("\r\n");
    assertStrictEquals(decode(actualCmds), client);
  },
  `250-mx.google.com at your service
250-SIZE 35651584
250 8BITMIME
250 Sender OK
221 Goodbye
`,
);

testExtensions(
  "ehlo smtputf8",
  async (c, bcmdbuf, cmdbuf) => {
    const basicClient = `EHLO localhost
MAIL FROM:<user+📧@gmail.com> SMTPUTF8
QUIT
`;

    await c.hello("localhost");
    assertStrictEquals(
      await c.extension("8BITMIME"),
      null,
      "Shoultn't support 8BITMIME",
    );
    assert(await c.extension("SMTPUTF8") != null, "Should support SMTPUTF8");
    await c.mail("user+📧@gmail.com");
    await c.quit();

    await bcmdbuf.flush();
    const actualcmds = await Deno.readAll(cmdbuf);
    const client = basicClient.split("\n").join("\r\n");
    assertStrictEquals(decode(actualcmds), client);
  },
  `250-mx.google.com at your service
250-SIZE 35651584
250 SMTPUTF8
250 Sender OK
221 Goodbye
`,
);

testExtensions(
  "ehlo 8bitmime smtputf8",
  async (c, bcmdbuf, cmdbuf) => {
    const basicClient = `EHLO localhost
MAIL FROM:<user+📧@gmail.com> BODY=8BITMIME SMTPUTF8
QUIT
`;

    await c.hello("localhost");
    c._didHello = true;
    assert(await c.extension("8BITMIME") != null, "Should support 8BITMIME");
    assert(await c.extension("SMTPUTF8") != null, "Should support SMTPUTF8");
    await c.mail("user+📧@gmail.com");
    await c.quit();

    await bcmdbuf.flush();
    const actualCmds = await Deno.readAll(cmdbuf);
    const client = basicClient.split("\n").join("\r\n");
    assertStrictEquals(decode(actualCmds), client);
  },
  `250-mx.google.com at your service
250-SIZE 35651584
250-8BITMIME
250 SMTPUTF8
250 Sender OK
221 Goodbye
	`,
);

Deno.test("[smtp] createSMTPClient", async () => {
  const server = `220 hello world
250-mx.google.com at your service
250-SIZE 35651584
250-AUTH LOGIN PLAIN
250 8BITMIME
221 OK
`.split("\n").join("\r\n");
  const client = `EHLO localhost
QUIT
`.split("\n").join("\r\n");

  const cmdbuf = new Deno.Buffer();
  const bcmdbuf = BufWriter.create(cmdbuf);
  const fakeConn = createFakeConn(
    BufReader.create(new StringReader(server)),
    bcmdbuf,
  );
  const c = await createSMTPClient(fakeConn, "fake.host");
  try {
    const authExt = await c.extension("aUtH");
    assertStrictEquals(authExt, "LOGIN PLAIN");
    assertStrictEquals(await c.extension("DSN"), null, "Shouldn't support DSN");
    await c.quit();

    await bcmdbuf.flush();
    const actualCmds = await Deno.readAll(cmdbuf);
    assertStrictEquals(decode(actualCmds), client);
  } finally {
    c.close();
  }
});

Deno.test("[smtp] createSMTPClient2", async () => {
  const server = `220 hello world
502 EH?
250-mx.google.com at your service
250-SIZE 35651584
250-AUTH LOGIN PLAIN
250 8BITMIME
221 OK
`.split("\n").join("\r\n");
  const client = `EHLO localhost
HELO localhost
QUIT
`.split("\n").join("\r\n");

  const cmdbuf = new Deno.Buffer();
  const bcmdbuf = BufWriter.create(cmdbuf);
  const fakeConn = createFakeConn(
    BufReader.create(new StringReader(server)),
    bcmdbuf,
  );
  const c = await createSMTPClient(fakeConn, "fake.host");
  try {
    assertStrictEquals(await c.extension("DSN"), null, "Shouldn't support DSN");
    await c.quit();

    await bcmdbuf.flush();
    const actualCmds = await Deno.readAll(cmdbuf);
    assertStrictEquals(decode(actualCmds), client);
  } catch (e) {
    c.close();
  }
});

Deno.test({
  ignore: true,
  name: "[smtp] createSMTPClient with TLS",
  fn: async () => {},
});

Deno.test("[smtp] hello", async () => {
  const baseHelloServer = `220 hello world
502 EH?
250-mx.google.com at your service
250 FEATURE
`;

  const helloServer = [
    "",
    "502 Not implemented\n",
    "250 User is valid\n",
    "235 Accepted\n",
    "250 Sender ok\n",
    "",
    "250 Reset ok\n",
    "221 Goodbye\n",
    "250 Sender ok\n",
    "250 ok\n",
  ];

  const baseHelloClient = `EHLO customhost
HELO customhost
`;

  const helloClient = [
    "",
    "STARTTLS\n",
    "VRFY test@example.com\n",
    "AUTH PLAIN AHVzZXIAcGFzcw==\n",
    "MAIL FROM:<test@example.com>\n",
    "",
    "RSET\n",
    "QUIT\n",
    "VRFY test@example.com\n",
    "NOOP\n",
  ];

  assertStrictEquals(
    helloServer.length,
    helloClient.length,
    "Hello server and client size mismatch",
  );

  for (let i = 0; i < helloServer.length; i++) {
    const server = (baseHelloServer + helloServer[i]).split("\n").join("\r\n");
    const client = (baseHelloClient + helloClient[i]).split("\n").join("\r\n");
    const cmdbuf = new Deno.Buffer();
    const bcmdbuf = BufWriter.create(cmdbuf);
    const fakeConn = createFakeConn(
      BufReader.create(new StringReader(server)),
      bcmdbuf,
    );
    const c = await createSMTPClient(
      fakeConn,
      "fake.host",
    );
    assert(c instanceof SMTPClientImpl);
    try {
      c._localName = "customhost";
      switch (i) {
        case 0:
          await assertFailsAsync(
            () =>
              c.hello(
                "hostinjection>\n\rDATA\r\nInjected message body\r\n.\r\nQUIT\r\n",
              ),
            "Expected Hello to be rejected due to a message injection attempt",
          );
          await c.hello("customhost");
          break;
        case 1:
          await assertFailsAsync(() => c.startTLS({}), "502 Not implemented");
          break;
        case 2:
          await c.verify("test@example.com");
          break;
        case 3:
          c._isTLS = true;
          c._serverName = "smtp.google.com";
          await c.auth(plainAuth({
            identity: "",
            username: "user",
            password: "pass",
            host: "smtp.google.com",
          }));
          break;
        case 4:
          await c.mail("test@example.com");
          break;
        case 5:
          assertStrictEquals(
            await c.extension("feature"),
            null,
            "Expected FEATURE not to be supported",
          );
          break;
        case 6:
          await c.reset();
          break;
        case 7:
          await c.quit();
          break;
        case 8:
          try {
            await c.verify("test@example.com");
          } catch (err) {
            await c.hello("customhost");
          }
          break;
        case 9:
          await c.noop();
          break;
        default:
          throw new Error("Unhandled command");
      }

      await bcmdbuf.flush();
      const actualCmds = await Deno.readAll(cmdbuf);
      assertStrictEquals(decode(actualCmds), client);
    } finally {
      c.close();
    }
  }
});

Deno.test("[smtp] sendMail", async () => {
  const server = `220 hello world
502 EH?
250 mx.google.com at your service
250 Sender ok
250 Receiver ok
354 Go ahead
250 Data ok
221 Goodbye
`.split("\n").join("\r\n");
  const client = `EHLO localhost
HELO localhost
MAIL FROM:<test@example.com>
RCPT TO:<other@example.com>
DATA
From: test@example.com
To: other@example.com
Subject: SendMail test
SendMail is working for me.
.
QUIT
`.split("\n").join("\r\n");

  const cmdbuf = new Deno.Buffer();
  const bcmdbuf = BufWriter.create(cmdbuf);
  const port = 4507;
  const l = Deno.listen({ transport: "tcp", port });
  const { addr } = l;
  assert(addr.transport === "tcp");
  try {
    const done = deferred<void>();
    const cancel = deferred<void>();
    const timeout = setTimeout(() => {
      cancel.reject(new Error("Timeout"));
    }, 10000);

    (async (data: string[]) => {
      const conn = await l.accept();
      try {
        const tc = new TextProtoConn(conn);
        for (let i = 0; i < data.length && Boolean(data[i]); i++) {
          await tc.printLine(data[i]);

          while (data[i].length >= 4 && data[i][3] === "-") {
            i++;
            await tc.printLine(data[i]);
          }

          if (data[i] === "221 Goodbye") {
            break;
          }

          let read = false;
          while (!read || data[i] === "354 Go ahead") {
            const msg = await tc.readLine();
            await bcmdbuf.write(encode((msg ?? "") + "\r\n"));
            read = true;
            if (data[i] === "354 Go ahead" && msg === ".") {
              break;
            }
          }
        }
      } finally {
        conn.close();
      }

      done.resolve();
    })(server.split("\r\n"));

    await assertFailsAsync(
      () =>
        sendMail({
          addr,
          from: "test@example.com",
          to: [
            "other@example.com>\n\rDATA\r\nInjected message body\r\n.\r\nQUIT\r\n",
          ],
          msg: `From: test@example.com
To: other@example.com
Subject: SendMail test
SendMail is working for me.
`.replaceAll("\n", "\r\n"),
        }),
      "Expected sendMail to be rejected due to a message injection attempt",
    );

    await sendMail({
      addr,
      from: "test@example.com",
      to: ["other@example.com"],
      msg: `From: test@example.com
To: other@example.com
Subject: SendMail test
SendMail is working for me.
`.replaceAll("\n", "\r\n"),
    });

    await Promise.race([done, cancel]);
    clearTimeout(timeout);

    await bcmdbuf.flush();
    const actualCmds = await Deno.readAll(cmdbuf);
    assertStrictEquals(decode(actualCmds), client);
  } finally {
    l.close();
  }
});

Deno.test({
  ignore: true,
  name: "[smtp] sendMail with AUTH",
  fn: async () => {},
});

Deno.test({
  ignore: true,
  name: "[smtp] auth failed",
  fn: async () => {},
});

Deno.test({
  ignore: true,
  name: "[smtp] tls client",
  fn: async () => {},
});
