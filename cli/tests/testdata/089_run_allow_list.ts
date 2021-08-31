try {
  Deno.run({
    cmd: ["ls"],
  });
} catch (e) {
  console.log(e);
}

const proc = Deno.run({
  cmd: ["curl", "--help"],
  stdout: "null",
});
console.log((await proc.status()).success);
