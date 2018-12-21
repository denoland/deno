import { cwd, readDirSync, run } from "deno";

const rootPath = (): string => {
  // remove '/js' in end of cwd to go on root path
  return cwd();
};

const clangFormatPath = (): string => {
  return rootPath() + "/third_party/depot_tools/clang-format";
};

const joinPath = (joinSet: string[]): string => {
  return joinSet.join("/");
};

const clangFormat = () => {
  console.log("clang Format");
  run({
    args: [
      clangFormatPath(),
      "-i",
      "-style",
      "Google",
    ].concat(findExts(rootPath() + '/libdeno'))
  });
};

// this should return array of file path which has extension '.cc' and '.h'
const findExts = (path: string): string[] => {
  const files = readDirSync(path)
  const fileWithTargetExts = files.filter((item) => {
    return item.name.endsWith('.cc') || item.name.endsWith('.h')
  }).map((item) => {
    return path + '/' + item.name
  })

  return fileWithTargetExts
}

const gnFormat = () => {
  console.log("gn Format");
};

const yapf = () => {
  console.log("yapf");
};

const prettier = () => {
  console.log("prettier");
};

const rustfmt = () => {
  console.log("rustfmt");
};

function format() {
  const prettier = joinPath([
    rootPath(),
    "third_party",
    "node_modules",
    "prettier",
    "bin-prettier.js"
  ]);
  const toolsPath = joinPath([rootPath(), "tools"]);
  const rustfmtConfig = joinPath([toolsPath, "rustfmt.toml"]);
  clangFormat();
}

format();
