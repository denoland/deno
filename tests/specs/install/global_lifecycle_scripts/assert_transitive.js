const messagePath =
  "./bins-transitive/bin/.transitive-lifecycle/node_modules/.deno/@denotest+transitive-lifecycle-script@1.0.0/node_modules/@denotest/transitive-lifecycle-script/transitive_message.js";
const message = Deno.readTextFileSync(messagePath);

if (!message.includes("transitive postinstall works")) {
  throw new Error("transitive postinstall script did not run");
}
