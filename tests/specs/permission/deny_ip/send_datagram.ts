const deno = new Deno.Command(Deno.execPath(), {
  args: [
    "eval",
    "--unstable-net",
    "const l = Deno.listenDatagram({ hostname: '0.0.0.0', port: 4540, transport: 'udp' }); console.log('ready'); await l.receive(); await l.close();",
  ],
  stdout: "piped",
}).spawn();

const p = Promise.withResolvers<void>();
for await (const chunk of deno.stdout) {
  const text = new TextDecoder().decode(chunk);
  if (text.includes("ready")) {
    p.resolve();
    break;
  }
}

const timeout = setTimeout(() => {
  console.log("timed out");
  p.reject(new Error("timed out"));
}, 15000);

await p.promise;
clearTimeout(timeout);

console.log("listener ready");

const l = Deno.listenDatagram({
  hostname: "0.0.0.0",
  port: 0,
  transport: "udp",
});

try {
  console.log("sending");
  await l.send(new TextEncoder().encode("hello"), {
    port: 4540,
    transport: "udp",
  });
  console.log("sent");
} catch (e) {
  console.error("errored: ", e);
}

l.close();
deno.kill();
