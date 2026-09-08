// A registry dependency and a workspace member both declare a `thing-bin`
// executable. The dependency must win: silently shadowing a real dependency's
// executable with a local member's would be surprising, and npm links the
// dependency too.

function binTargets(binPath: string): string[] {
  if (Deno.build.os !== "windows") {
    Deno.lstatSync(binPath);
    return [binPath];
  }
  const dir = binPath.slice(0, binPath.lastIndexOf("/"));
  const text = Deno.readTextFileSync(binPath);
  return [...text.matchAll(/\$basedir\/([^"]+)/g)].map((m) => `${dir}/${m[1]}`);
}

const resolved = binTargets("node_modules/.bin/thing-bin").map((path) => {
  try {
    return Deno.realPathSync(path);
  } catch {
    return null;
  }
});

for (const dir of ["thing", "thing-alt"]) {
  const member = Deno.realPathSync(`packages/${dir}/cli.js`);
  if (resolved.includes(member)) {
    throw new Error(
      `node_modules/.bin/thing-bin should point at the @denotest/one-bin ` +
        `dependency, not the workspace member (${member})`,
    );
  }
}
if (!resolved.some((path) => path?.replaceAll("\\", "/").includes("one-bin"))) {
  throw new Error(
    `expected node_modules/.bin/thing-bin to point into @denotest/one-bin, ` +
      `got ${JSON.stringify(resolved)}`,
  );
}

console.log("ok");
