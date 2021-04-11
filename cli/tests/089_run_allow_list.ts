try {
  Deno.run({
    cmd: ["ls"],
  });
} catch (e) {
  console.log(e);
}

const proc = Deno.run({
  cmd: ["cat", "089_run_allow_list.ts"],
  stdout: "null",
});
console.log((await proc.status()).success);
