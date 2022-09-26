console.log(Location);
console.log(Location.prototype);
console.log(location);

globalThis.location = {
  hash: "#bat",
  host: "foo",
  hostname: "foo",
  href: "https://foo/bar?baz#bat",
  origin: "https://foo",
  pathname: "/bar",
  port: "",
  protocol: "https:",
  search: "?baz",
};
console.log(location.pathname);
