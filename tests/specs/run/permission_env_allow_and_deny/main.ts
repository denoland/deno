const obj = Deno.env.toObject();
if (obj["PATH"] == null) {
  throw "FAIL";
}
if ("FOOBAR" in obj) {
  throw "FAIL2";
}
