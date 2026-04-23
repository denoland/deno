const obj = Deno.env.toObject();
const pathKey = Object.keys(obj).find((p) => p.toLowerCase() === "path");
if (pathKey == null) {
  throw "FAIL CASING";
}
if (obj[pathKey] == null) {
  throw "FAIL";
}
if ("FOOBAR" in obj) {
  throw "FAIL2";
}
