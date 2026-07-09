const realPath = Deno.realPathSync(
  "c/node_modules/@denotest/lifecycle-scripts-counter",
);
const packagePathSuffix = `${
  Deno.build.os === "windows" ? "\\" : "/"
}node_modules${Deno.build.os === "windows" ? "\\" : "/"}@denotest${
  Deno.build.os === "windows" ? "\\" : "/"
}lifecycle-scripts-counter`;
const packageFolder = realPath.slice(
  0,
  realPath.lastIndexOf(packagePathSuffix),
);
Deno.removeSync(packageFolder, { recursive: true });
