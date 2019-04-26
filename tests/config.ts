const map = new Map<string, { foo: string }>();

if (map.get("bar").foo) {
  console.log("here");
}
