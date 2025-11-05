console.log(Deno.env.get("VAR1"));
console.log(Deno.env.get("VAR2"));
const object = Deno.env.toObject();
console.log(object);
if ("VAR1" in object) {
  throw "FAILED";
}
