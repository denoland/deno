try {
  Deno.run({
    cmd: ["ls"],
  });
} catch (e) {
  console.log(e);
}

const proc = Deno.run({
  cmd: Deno.build.os === "windows"
    ? ["cmd", "/c", "type", "089_run_allow_list.ts"]
    : ["cat", "089_run_allow_list.ts"],
  stdout: "null",
});
console.log((await proc.status()).success);
