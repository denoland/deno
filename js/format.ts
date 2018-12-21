import { cwd, readDirSync, run } from "deno";

const rootPath = (): string => {
  // remove '/js' in end of cwd to go on root path
  return cwd();
};

// this should return array of file path which has extension in 'exts'
const findExts = (path: string, exts: string[]): string[] => {
  const files = readDirSync(path)
  let fileWithTargetExts = []
  files.forEach((file) => {
    for (let ext of exts) {
      if (file.name.endsWith(ext)) {
        fileWithTargetExts.push(path + '/' + file.name)
      }
    }
  })

  return fileWithTargetExts
}

const clangFormatPath = (): string => {
  return rootPath() + "/third_party/depot_tools/clang-format";
};

const gnFormatPath = (): string => {
  return rootPath() + "/third_party/depot_tools/gn";
}

const joinPath = (joinSet: string[]): string => {
  return joinSet.join("/");
};

const clangFormat = () => {
  console.log("clang Format");
  /*
  run({
    args: [
      clangFormatPath(),
      "-i",
      "-style",
      "Google",
    ].concat(findExts(rootPath() + '/libdeno', ['.cc', '.h']))
  });
  */
};

const gnFilePaths = [
  '',
  '/build_extra/rust',
  '/build_extra/flatbuffers',
  '/build_extra/flatbuffers/rust',
  '/libdeno'
]

const gnFormat = () => {
  console.log("gn Format");
  let filesToFormat = [];
  gnFilePaths.forEach((path) => {
    filesToFormat = filesToFormat.concat(findExts(rootPath() + path, ['.gn', '.gni']))
  })
  
  for (let file of filesToFormat) {
    console.log(file)
    /*
    run({
      args: [
        gnFormatPath(),
        "format",
        file
      ]
      // TODO: google_env
    });
    */
  }
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
  // clangFormat();
  gnFormat();
}

format();
