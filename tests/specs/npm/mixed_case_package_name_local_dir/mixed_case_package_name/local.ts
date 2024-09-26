import value from "npm:@denotest/MixedCase";
console.log(value);
console.log(pathExists("./node_modules/.deno"));
console.log(
  pathExists("./node_modules/.deno/_ibsgk3tporsxg5bpinavaskuifgfg@1.0.0"),
);

function pathExists(filePath: string) {
  try {
    Deno.lstatSync(filePath);
    return true;
  } catch (error) {
    if (error instanceof Deno.errors.NotFound) {
      return false;
    }
    throw error;
  }
}
