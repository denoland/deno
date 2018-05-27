
const request = async () => {
  const response = await fetch('http://localhost:4545/package.json');
  const json = await response.json();
  console.log("expect deno:", json.name);
  if (json.name !== "deno") {
    throw Error("bad value" + json.name);
  }
}

request();
console.log("fetch started");
