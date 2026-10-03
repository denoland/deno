// The write grant is the cwd recorded at process start; Deno.chdir does not
// widen it. After chdir("/"), a relative write resolves outside the grant and
// is denied, while writes back under the original cwd still succeed.
const original = Deno.cwd();
Deno.chdir("/");
try {
  Deno.writeTextFileSync("./relaxed_chdir.txt", "data");
  console.log("write after chdir: UNEXPECTEDLY ALLOWED");
} catch (err) {
  console.log(`write after chdir: ${err.name}`);
}
Deno.writeTextFileSync(`${original}/still_ok.txt`, "data");
console.log("write original cwd: ok");
