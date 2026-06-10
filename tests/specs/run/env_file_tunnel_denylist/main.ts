// The denylisted Deno control variables must not have been imported from the
// env file...
console.log("DENO_CONNECTED:", Deno.env.get("DENO_CONNECTED"));
console.log(
  "DENO_DEPLOY_TUNNEL_ENDPOINT:",
  Deno.env.get("DENO_DEPLOY_TUNNEL_ENDPOINT"),
);
// ...but ordinary variables from the same file still are.
console.log("NOT_DENYLISTED:", Deno.env.get("NOT_DENYLISTED"));
console.log("user code started");
