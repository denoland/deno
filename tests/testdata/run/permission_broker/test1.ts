Deno.readTextFileSync("./run/permission_broker/scratch.txt");
Deno.readTextFileSync("./run/permission_broker/scratch.txt");
Deno.readTextFileSync("./run/permission_broker/log.txt");
Deno.writeTextFileSync(
  "./run/permission_broker/log.txt",
  "Lorem ipsum dolor sit amet",
);
console.log("env", Deno.env.toObject());
