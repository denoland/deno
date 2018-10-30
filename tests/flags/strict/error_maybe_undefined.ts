const map = new Map<string, { bar: string }>();

if (map.get("foo").bar) {
  console.log("maybe undefined");
}
