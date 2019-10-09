const { dial, run } = Deno;

import { test, runIfMain } from "../testing/mod.ts";
import { assert, assertEquals } from "../testing/asserts.ts";
import { BufReader } from "../io/bufio.ts";
import { TextProtoReader } from "../textproto/mod.ts";

let server: Deno.Process;
async function startServer(): Promise<void> {
  server = run({
    args: [Deno.execPath(), "run", "-A", "http/racing_server.ts"],
    stdout: "piped"
  });
  // Once racing server is ready it will write to its stdout.
  const r = new TextProtoReader(new BufReader(server.stdout!));
  const s = await r.readLine();
  assert(s !== Deno.EOF && s.includes("Racing server listening..."));
}
function killServer(): void {
  server.close();
  server.stdout!.close();
}

const input = `GET / HTTP/1.1

GET / HTTP/1.1

GET / HTTP/1.1

GET / HTTP/1.1

`;
const HUGE_BODY_SIZE = 1024 * 1024;
const output = `HTTP/1.1 200 OK
content-length: 8

Hello 1
HTTP/1.1 200 OK
content-length: ${HUGE_BODY_SIZE}

${"a".repeat(HUGE_BODY_SIZE)}HTTP/1.1 200 OK
content-length: ${HUGE_BODY_SIZE}

${"b".repeat(HUGE_BODY_SIZE)}HTTP/1.1 200 OK
content-length: 8

World 4
`;

test(async function serverPipelineRace(): Promise<void> {
  await startServer();

  const conn = await dial({ port: 4501 });
  const r = new TextProtoReader(new BufReader(conn));
  await conn.write(new TextEncoder().encode(input));
  const outLines = output.split("\n");
  // length - 1 to disregard last empty line
  for (let i = 0; i < outLines.length - 1; i++) {
    const s = await r.readLine();
    assertEquals(s, outLines[i]);
  }
  killServer();
});

runIfMain(import.meta);
