import inspector from "node:inspector/promises";
import vm from "node:vm";

const session = new inspector.Session();
session.connect();
await session.post("Profiler.enable");
await session.post("Profiler.startPreciseCoverage", {
  callCount: true,
  detailed: true,
});

const script = new vm.Script('console.log("hello world")', {
  filename: "/some/path/to/file/users-source-code.js",
});

script.runInThisContext();

const coverage = await session.post("Profiler.takePreciseCoverage");
const results = coverage.result.filter((scriptCoverage) =>
  scriptCoverage.url.includes("users-source-code")
);

console.log(JSON.stringify(results, null, 2));
