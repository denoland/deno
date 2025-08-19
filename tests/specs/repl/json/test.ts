const {
  kExtraStdio,
  getExtraPipeRids,
  writableStreamForRid,
  readableStreamForRid,
} = Deno[Deno.internal];

const command = new Deno.Command(Deno.execPath(), {
  args: [
    "repl",
    "--json",
  ],
  stdio: "null",
  stderr: "inherit",
  stdout: "inherit",
  [kExtraStdio]: ["piped"],
});

await using child = command.spawn();

const pipeRid = getExtraPipeRids(child)[0];
const writable = writableStreamForRid(pipeRid);
const readable = readableStreamForRid(pipeRid);

{
  const writer = writable.getWriter();
  const buf = new TextEncoder().encode(
    JSON.stringify({
      type: "Run",
      code: "let a = 1;",
      output: false,
    }),
  );
  const u32 = new Uint8Array(4);
  new DataView(u32.buffer).setUint32(0, buf.length, true);
  await writer.write(u32);
  await writer.write(buf);
  writer.releaseLock();
}

{
  const reader = readable.getReader({ mode: "byob" });
  const { value: u32 } = await reader.read(new Uint8Array(4), { min: 4 });
  const { value } = await reader.read(
    new Uint8Array(new DataView(u32.buffer).getUint32(0, true)),
  );
  console.log(JSON.parse(new TextDecoder().decode(value)));
  reader.releaseLock();
}

{
  const writer = writable.getWriter();
  const buf = new TextEncoder().encode(
    JSON.stringify({
      type: "Run",
      code: "console.log('hello'); a + 1",
      output: true,
    }),
  );
  const u32 = new Uint8Array(4);
  new DataView(u32.buffer).setUint32(0, buf.length, true);
  await writer.write(u32);
  await writer.write(buf);
  writer.releaseLock();
}

{
  const reader = readable.getReader({ mode: "byob" });
  const { value: u32 } = await reader.read(new Uint8Array(4), { min: 4 });
  const { value } = await reader.read(
    new Uint8Array(new DataView(u32.buffer).getUint32(0, true)),
  );
  console.log(JSON.parse(new TextDecoder().decode(value)));
  reader.releaseLock();
}

{
  const writer = writable.getWriter();
  const buf = new TextEncoder().encode(
    JSON.stringify({
      type: "Run",
      code: "throw new Error('hi')",
      output: true,
    }),
  );
  const u32 = new Uint8Array(4);
  new DataView(u32.buffer).setUint32(0, buf.length, true);
  await writer.write(u32);
  await writer.write(buf);
  writer.releaseLock();
}

{
  const reader = readable.getReader({ mode: "byob" });
  const { value: u32 } = await reader.read(new Uint8Array(4), { min: 4 });
  const { value } = await reader.read(
    new Uint8Array(new DataView(u32.buffer).getUint32(0, true)),
  );
  console.log(JSON.parse(new TextDecoder().decode(value)));
  reader.releaseLock();
}
