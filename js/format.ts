import { cwd, run } from "deno";

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
  console.log("clang_format");
  run({
    args: [
      clangFormatPath(),
      "-i",
      "-style",
      "Google",
      findExts(rootPath + '/libdeno', ['cc', 'h'])
    ]
  });
};

// this should return array of file path which has certain extensions
const findExts = (path: string, ext: string[]): string => {
  return ''
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
