import { checkHostname } from "./bar.ts";

function hello() {
  console.log("Hello there!", checkHostname());
}

function bar() {
  return hello();
}

bar();

const listener = Deno.listen({ hostname: "0.0.0.0", port: 8080 });
const conn = await listener.accept();
