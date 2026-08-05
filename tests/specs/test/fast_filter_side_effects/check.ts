let bodyRan = true;
try {
  Deno.readTextFileSync("body_ran.txt");
} catch {
  bodyRan = false;
}
if (bodyRan) {
  throw new Error("test body of a filtered-out file was executed");
}
const marker = Deno.readTextFileSync("top_level_marker.txt");
if (marker !== "x") {
  throw new Error(
    `top-level code of a filtered-out file ran ${marker.length} times, expected exactly once`,
  );
}
console.log("checks passed");
