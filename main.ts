import { checkHostname } from "./bar.ts";

function hello() {
  console.log("Hello there!", checkHostname());
}

function bar() {
  return hello();
}

bar();
