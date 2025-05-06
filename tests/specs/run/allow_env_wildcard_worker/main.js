console.log("main1", Deno.env.get("DENO_HELLO"));
console.log("main2", Deno.env.get("DENO_BYE"));
console.log("main3", Deno.env.get("AWS_HELLO"));

new Worker(import.meta.resolve("./worker.js"), {
  type: "module",
  deno: {
    permissions: {
      env: ["DENO_*"],
    },
  },
});
