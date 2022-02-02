import {
  assert,
  assertStrictEquals,
} from "https://deno.land/std@0.123.0/testing/asserts.ts";

if (Deno.args[0] !== "--child") {
  // Parent process.
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "run",
      "--unstable",
      import.meta.url,
      "--child",
    ],
    ipc: true,
  });

  await writeText(p.ipc, "ping");
  assertStrictEquals(await readText(p.ipc), "pong");
  await writeText(p.ipc, "hello");
  assertStrictEquals(await readText(p.ipc), "world");

  p.ipc.close();
  await p.status();
} else {
  // Child process.
  assert(Deno.ipc);
  assertStrictEquals(await readText(Deno.ipc), "ping");
  await writeText(Deno.ipc, "pong");
  assertStrictEquals(await readText(Deno.ipc), "hello");
  await writeText(Deno.ipc, "world");
}

async function readText(conn: Deno.Conn) {
  const buf = new Uint8Array(1024);
  const nread = await conn.read(buf);
  return nread != null
    ? new TextDecoder().decode(buf.subarray(0, nread))
    : null;
}

async function writeText(conn: Deno.Conn, text: string) {
  const buf = new TextEncoder().encode(text);
  await conn.write(buf);
}
