function printName(name: string, level = 0) {
  console.log("-".repeat(level) + " " + name);
}

function printDir(dir: string, level = 0, maxLevel = 0) {
  if (maxLevel > 0 && level > maxLevel) {
    return;
  }
  const files = Deno.readDirSync(dir).toArray();
  files.sort((a, b) => a.name.localeCompare(b.name));
  for (const file of files) {
    if (file.isDirectory) {
      printName(file.name, level);
      printDir(dir + "/" + file.name, level + 1, maxLevel);
    } else if (file.isFile) {
      printName(file.name, level);
    } else {
      printName(file.name + " (symlink)", level);
    }
  }
}

let maxLevel = 0;
for (let i = 0; i < Deno.args.length; i++) {
  const arg = Deno.args[i];
  if (arg.startsWith("--max-level=")) {
    maxLevel = parseInt(arg.split("=")[1]);
  } else {
    console.log(arg);
    printDir(arg, 1, maxLevel);
    if (i < Deno.args.length - 1) {
      console.log("");
      maxLevel = 0;
    }
  }
}
