let fileText = "let myVar = 0;\n";

for (var i = 0; i < 8192; i++) {
  fileText += "myVar += 1\n";
}

fileText += "console.log(myVar); // make this line longer with a comment\n";

Deno.writeTextFileSync("main.ts", fileText);
