await outputPermission("read");
await outputPermission("write");

self.close();

async function outputPermission(permission: "read" | "write") {
  const result = await Deno.permissions.query({
    name: permission,
    path: "./file.txt",
  });
  console.log(result.state === "granted");
}
