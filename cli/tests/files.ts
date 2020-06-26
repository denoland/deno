const filepaths = [
  "ampersand_&.txt",
  "at_@.txt",
  "emoji_🙃.txt",
  "percent_%.txt",
  "pound_#.txt",
  "space_ .txt",
  "file:/file",
  "file://file",
  "file:///file",
  "file:/root/file",
  "file://root/file",
  "file:///root/file",
  "file:/root/directory/file",
  "file://root/directory/file",
  "file:///root/directory/file",
];

Deno.chdir("cli/tests/files");

for (const filepath of filepaths) {
  const file = await Deno.open(filepath, { read: true });
  await Deno.copy(file, Deno.stdout);
  file.close();
}
