const fixtureFile = await Deno.makeTempFile();
let prefix = "file://";
if (Deno.build.os == "windows") {
  prefix += "/";
}
const fixtureUrl = new URL(`${prefix}${fixtureFile}`);
let resolve;

let p = new Promise((res) => resolve = res);

await Deno.writeTextFile(fixtureUrl, `self.postMessage("hello");\n`);

const workerA = new Worker(fixtureUrl.href, { type: "module" });
workerA.onmessage = (msg) => {
  console.log(msg.data);
  resolve();
};

await p;
workerA.terminate();

p = new Promise((res) => resolve = res);

await Deno.writeTextFile(fixtureUrl, `self.postMessage("goodbye");\n`);

const workerB = new Worker(fixtureUrl.href, { type: "module" });
workerB.onmessage = (msg) => {
  console.log(msg.data);
  resolve();
};

await p;
workerB.terminate();
