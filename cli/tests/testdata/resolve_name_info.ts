const [ssh, http, https] = [
  Deno.resolveNameInfo("127.0.0.1", 22),
  Deno.resolveNameInfo("127.0.0.1", 80),
  Deno.resolveNameInfo("127.0.0.1", 443),
];

console.log("SSH");
console.log(JSON.stringify(ssh));

console.log("HTTP");
console.log(JSON.stringify(http));

console.log("HTTPS");
console.log(JSON.stringify(https));

try {
  Deno.resolveNameInfo("not-an-ip", 9999);
} catch (e) {
  console.log(
    `Error ${e instanceof Error ? e.name : "[non-error]"} thrown for not-an-ip`,
  );
}
