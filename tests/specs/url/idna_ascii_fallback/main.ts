// Tests for the WHATWG URL #914 ASCII "domain to ASCII" fallback: an all-ASCII
// domain whose Unicode ToASCII step fails (e.g. a bogus `xn--` label) is
// accepted as the lowercased percent-decoded host as-is rather than throwing.

const BOGUS = "xn--72czcrhaj7cpt0ed1dxb4mb1s1.blogspot.com";

function log(label: string, value: unknown) {
  console.log(`${label}: ${value}`);
}

function throws(fn: () => unknown): string {
  try {
    fn();
    return "no throw";
  } catch (e) {
    return `throws ${(e as Error).constructor.name}`;
  }
}

console.log("== parse ==");
{
  const u = new URL(`https://${BOGUS}`);
  log("hostname", u.hostname);
  log("href", u.href);
  log("roundtrip", new URL(u.href).hostname);
}
{
  // Uppercase host is lowercased.
  const u = new URL(`https://XN--72CZCRHAJ7CPT0ED1DXB4MB1S1.BLOGSPOT.COM/`);
  log("upper.hostname", u.hostname);
}
{
  // userinfo + port + path + query + fragment all preserved.
  const u = new URL(`https://user:pass@${BOGUS}:8080/p/q?x=1#frag`);
  log("full.href", u.href);
  log("full.username", u.username);
  log("full.password", u.password);
  log("full.hostname", u.hostname);
  log("full.port", u.port);
  log("full.pathname", u.pathname);
  log("full.search", u.search);
  log("full.hash", u.hash);
}
{
  log("URL.parse", URL.parse(`https://${BOGUS}/`)?.hostname);
  log("URL.canParse", URL.canParse(`https://${BOGUS}/`));
}

console.log("== scheme-relative against a base ==");
{
  const u = new URL(`//${BOGUS}/path?q#f`, "https://base.example/");
  log("rel.protocol", u.protocol);
  log("rel.hostname", u.hostname);
  log("rel.pathname", u.pathname);
  log("rel.search", u.search);
  log("rel.hash", u.hash);
  log(
    "rel.canParse",
    URL.canParse(`//${BOGUS}/path?q#f`, "https://base.example/"),
  );
}
{
  const u = new URL(
    `//XN--72CZCRHAJ7CPT0ED1DXB4MB1S1.BLOGSPOT.COM/p`,
    "https://base.example/",
  );
  log("rel.upper.hostname", u.hostname);
}

console.log("== mutate a fallback-host URL ==");
{
  const u = new URL(`https://${BOGUS}/p?q#f`);
  u.pathname = "/changed";
  log("mut.pathname", u.href);
  u.hash = "#newhash";
  log("mut.hash", u.href);
  u.search = "?newq=1";
  log("mut.search", u.href);
  u.port = "9999";
  log("mut.port", u.href);
  u.hostname = "example.com";
  log("mut.hostname->normal", u.href);
}
{
  const u = new URL(`https://${BOGUS}/p`);
  u.hostname = "xn--bogus";
  log("mut.hostname->bogus", u.href);
}

console.log("== setter host-substring semantics (normal url) ==");
{
  const u = new URL("https://example.com:123/p");
  u.hostname = "xn--bogus/stuff";
  log("n.hostname=bogus/stuff", u.href);
}
{
  const u = new URL("https://example.com:123/p");
  u.hostname = "example.org/stuff";
  log("n.hostname=example.org/stuff", u.href);
}
{
  // The hostname state aborts on `:` (no port, host unchanged), exactly like
  // the valid-host control below.
  const u = new URL("https://example.com:123/p");
  u.hostname = "xn--bogus:8080";
  log("n.hostname=bogus:8080", u.href);
}
{
  const u = new URL("https://example.com:123/p");
  u.hostname = "example.org:8080";
  log("n.hostname=example.org:8080", u.href);
}
{
  const u = new URL("https://example.com:123/p");
  u.hostname = "xn--bogus?x";
  log("n.hostname=bogus?x", u.href);
}
{
  const u = new URL("https://example.com:123/p");
  u.hostname = "xn--bogus#x";
  log("n.hostname=bogus#x", u.href);
}
{
  const u = new URL("https://example.com/p");
  u.host = "xn--bogus:8080/path";
  log("n.host=bogus:8080/path", u.href);
}
{
  const u = new URL("https://example.com/p");
  u.host = "example.org:8080/path";
  log("n.host=example.org:8080/path", u.href);
}
{
  const u = new URL("https://example.com/p");
  u.host = "xn--bogus/stuff";
  log("n.host=bogus/stuff", u.href);
}

console.log("== setter host-substring semantics (fallback-host url) ==");
{
  const u = new URL(`https://${BOGUS}:123/p`);
  u.hostname = "xn--bogus/stuff";
  log("f.hostname=bogus/stuff", u.href);
}
{
  const u = new URL(`https://${BOGUS}:123/p`);
  u.hostname = "example.org/stuff";
  log("f.hostname=example.org/stuff", u.href);
}
{
  const u = new URL(`https://${BOGUS}:123/p`);
  u.hostname = "xn--bogus:8080";
  log("f.hostname=bogus:8080", u.href);
}
{
  const u = new URL(`https://${BOGUS}/p`);
  u.host = "xn--bogus:8080/path";
  log("f.host=bogus:8080/path", u.href);
}
{
  const u = new URL(`https://${BOGUS}/p`);
  u.host = "example.org:8080/path";
  log("f.host=example.org:8080/path", u.href);
}
{
  const u = new URL(`https://${BOGUS}/p`);
  u.host = "xn--bogus/stuff";
  log("f.host=bogus/stuff", u.href);
}

console.log("== invalid IPv4 still fails / no-ops ==");
log("ipv4.parse", throws(() => new URL("https://1.2.3.4.5/")));
{
  const u = new URL("https://example.com:123/p");
  u.hostname = "1.2.3.4.5";
  log("n.hostname=ipv4", u.href);
}
{
  const u = new URL(`https://${BOGUS}/p`);
  u.host = "1.2.3.4.5:999";
  log("f.host=ipv4", u.href);
}

console.log("== other still-throw / no-op ==");
// Non-ASCII domain failing punycode (bidi rule violation) still throws.
log("nonascii.parse", throws(() => new URL("https://0aא.com/")));
// Forbidden code point host (%23 -> #) still throws.
log("forbidden.parse", throws(() => new URL("https://ex%23ample.com/")));
{
  // Forbidden-value setter is a no-op.
  const u = new URL(`https://${BOGUS}/p`);
  u.hostname = "ex%23ample.com";
  log("f.hostname=forbidden", u.href);
}
