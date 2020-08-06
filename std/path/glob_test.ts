import { assert, assertEquals } from "../testing/asserts.ts";
import {
  GlobToRegExpOptions,
  globToRegExp,
  isGlob,
  joinGlobs,
  normalizeGlob,
} from "./glob.ts";
import { SEP } from "./mod.ts";

function match(
  glob: string,
  path: string,
  opts: GlobToRegExpOptions = {},
): boolean {
  if (opts.os == null) {
    const matchDarwin = path.match(
      globToRegExp(glob, { ...opts, os: "darwin" }),
    );
    if (matchDarwin) {
      assertEquals(matchDarwin.length, 1);
    }
    const matchLinux = path.match(globToRegExp(glob, { ...opts, os: "linux" }));
    if (matchLinux) {
      assertEquals(matchLinux.length, 1);
    }
    const matchWindows = path.match(
      globToRegExp(glob, { ...opts, os: "windows" }),
    );
    if (matchWindows) {
      assertEquals(matchWindows.length, 1);
    }
    return !!matchDarwin && !!matchLinux && !!matchWindows;
  } else {
    const match = path.match(globToRegExp(glob, opts));
    if (match) {
      assertEquals(match.length, 1);
    }
    return !!match;
  }
}

Deno.test({
  name: "[path] globToRegExp() Basic RegExp",
  fn(): void {
    assertEquals(globToRegExp(""), /^$/);
    assertEquals(globToRegExp("*.js", { os: "linux" }), /^[^/]*\.js\/*$/);
  },
});

Deno.test({
  name: "[path] globToRegExp() * (wildcard)",
  fn(): void {
    assert(match("*", "foo", { extended: false, globstar: false }));
    assert(match("*", "foo", { extended: false, globstar: false }));
    assert(match("f*", "foo", { extended: false, globstar: false }));
    assert(match("f*", "foo", { extended: false, globstar: false }));
    assert(match("*o", "foo", { extended: false, globstar: false }));
    assert(match("*o", "foo", { extended: false, globstar: false }));
    assert(match("u*orn", "unicorn", { extended: false, globstar: false }));
    assert(match("u*orn", "unicorn", { extended: false, globstar: false }));
    assert(!match("ico", "unicorn", { extended: false, globstar: false }));
    assert(match("u*nicorn", "unicorn", { extended: false, globstar: false }));
    assert(match("u*nicorn", "unicorn", { extended: false, globstar: false }));
  },
});

Deno.test({
  name: "[path] globToRegExp() ? (match one character)",
  fn(): void {
    assert(match("f?o", "foo", { extended: false, globstar: false }));
    assert(match("f?o?", "fooo", { extended: false, globstar: false }));
    assert(!match("f?oo", "foo", { extended: false, globstar: false }));
    assert(!match("?fo", "fooo", { extended: false, globstar: false }));
    assert(!match("f?oo", "foo", { extended: false, globstar: false }));
    assert(!match("foo?", "foo", { extended: false, globstar: false }));
  },
});

Deno.test({
  name: "[path] globToRegExp() [seq] (character range)",
  fn(): void {
    assert(match("fo[oz]", "foo", { extended: false, globstar: false }));
    assert(match("fo[oz]", "foz", { extended: false, globstar: false }));
    assert(!match("fo[oz]", "fog", { extended: false, globstar: false }));
    assert(match("fo[a-z]", "fob", { extended: false, globstar: false }));
    assert(!match("fo[a-d]", "fot", { extended: false, globstar: false }));
    assert(!match("fo[!tz]", "fot", { extended: false, globstar: false }));
    assert(match("fo[!tz]", "fob", { extended: false, globstar: false }));
  },
});

Deno.test({
  name: "[path] globToRegExp() [[:alnum:]] (character class in range)",
  fn(): void {
    assert(
      match(
        "[[:alnum:]]/bar.txt",
        "a/bar.txt",
        { extended: false, globstar: false },
      ),
    );
    assert(
      match(
        "[[:alnum:]abc]/bar.txt",
        "1/bar.txt",
        { extended: false, globstar: false },
      ),
    );
    assert(
      match(
        "[[:digit:]]/bar.txt",
        "1/bar.txt",
        { extended: false, globstar: false },
      ),
    );
    assert(
      match(
        "[[:digit:]b]/bar.txt",
        "b/bar.txt",
        { extended: false, globstar: false },
      ),
    );
    assert(
      match(
        "[![:digit:]b]/bar.txt",
        "a/bar.txt",
        { extended: false, globstar: false },
      ),
    );
    assert(
      !match(
        "[[:alnum:]]/bar.txt",
        "!/bar.txt",
        { extended: false, globstar: false },
      ),
    );
    assert(
      !match(
        "[[:digit:]]/bar.txt",
        "a/bar.txt",
        { extended: false, globstar: false },
      ),
    );
    assert(
      !match(
        "[[:digit:]b]/bar.txt",
        "a/bar.txt",
        { extended: false, globstar: false },
      ),
    );
  },
});

Deno.test({
  name: "[path] globToRegExp() {} (brace expansion)",
  fn(): void {
    assert(
      match("foo{bar,baaz}", "foobaaz", { extended: false, globstar: false }),
    );
    assert(
      match("foo{bar,baaz}", "foobar", { extended: false, globstar: false }),
    );
    assert(
      !match("foo{bar,baaz}", "foobuzz", { extended: false, globstar: false }),
    );
    assert(
      match("foo{bar,b*z}", "foobuzz", { extended: false, globstar: false }),
    );
  },
});

Deno.test({
  name: "[path] globToRegExp() Complex matches",
  fn(): void {
    assert(
      match(
        "http://?o[oz].b*z.com/{*.js,*.html}",
        "http://foo.baaz.com/jquery.min.js",
        { extended: false, globstar: false },
      ),
    );
    assert(
      match(
        "http://?o[oz].b*z.com/{*.js,*.html}",
        "http://moz.buzz.com/index.html",
        { extended: false, globstar: false },
      ),
    );
    assert(
      !match(
        "http://?o[oz].b*z.com/{*.js,*.html}",
        "http://moz.buzz.com/index.htm",
        { extended: false, globstar: false },
      ),
    );
    assert(
      !match(
        "http://?o[oz].b*z.com/{*.js,*.html}",
        "http://moz.bar.com/index.html",
        { extended: false, globstar: false },
      ),
    );
    assert(
      !match(
        "http://?o[oz].b*z.com/{*.js,*.html}",
        "http://flozz.buzz.com/index.html",
        { extended: false, globstar: false },
      ),
    );
  },
});

Deno.test({
  name: "[path] globToRegExp() ** (globstar)",
  fn(): void {
    assert(match("/foo/**", "/foo/bar.txt"));
    assert(match("/foo/**", "/foo/bar/baz.txt"));
    assert(!match("/foo/**", "/foo/bar/baz.txt", { globstar: false }));
    assert(match("/foo/**", "/foo/bar", { globstar: false }));
    assert(match("/foo/**/*.txt", "/foo/bar/baz.txt"));
    assert(match("/foo/**/*.txt", "/foo/bar/baz/qux.txt"));
    assert(match("/foo/**/bar.txt", "/foo/bar.txt"));
    assert(match("/foo/**/**/bar.txt", "/foo/bar.txt"));
    assert(match("/foo/**/*/baz.txt", "/foo/bar/baz.txt"));
    assert(match("/foo/**/*.txt", "/foo/bar.txt"));
    assert(match("/foo/**/**/*.txt", "/foo/bar.txt"));
    assert(match("/foo/**/*/*.txt", "/foo/bar/baz.txt"));
    assert(match("**/*.txt", "/foo/bar/baz/qux.txt"));
    assert(match("**/foo.txt", "foo.txt"));
    assert(match("**/*.txt", "foo.txt"));
    assert(!match("/foo/**.txt", "/foo/bar/baz/qux.txt"));
    assert(
      !match("/foo/bar**/*.txt", "/foo/bar/baz/qux.txt"),
    );
    assert(!match("/foo/bar**", "/foo/bar/baz.txt"));
    assert(!match("**/.txt", "/foo/bar/baz/qux.txt"));
    assert(
      !match(
        "http://foo.com/*",
        "http://foo.com/bar/baz/jquery.min.js",
      ),
    );
    assert(
      !match("http://foo.com/*", "http://foo.com/bar/baz/jquery.min.js"),
    );
    assert(
      match("http://foo.com/**", "http://foo.com/bar/baz/jquery.min.js"),
    );
    assert(
      match(
        "http://foo.com/**/jquery.min.js",
        "http://foo.com/bar/baz/jquery.min.js",
      ),
    );
    assert(
      !match(
        "http://foo.com/*/jquery.min.js",
        "http://foo.com/bar/baz/jquery.min.js",
      ),
    );
  },
});

Deno.test({
  name: "[path] globToRegExp() ?(pattern-list) (extended: match zero or one)",
  fn(): void {
    assert(match("?(foo).txt", "foo.txt"));
    assert(!match("?(foo).txt", "foo.txt", { extended: false }));
    assert(match("?(foo).txt", "a(foo).txt", { extended: false }));
    assert(match("?(foo).txt", ".txt"));
    assert(match("?(foo|bar)baz.txt", "foobaz.txt"));
    assert(match("?(ba[zr]|qux)baz.txt", "bazbaz.txt"));
    assert(match("?(ba[zr]|qux)baz.txt", "barbaz.txt"));
    assert(match("?(ba[zr]|qux)baz.txt", "quxbaz.txt"));
    assert(match("?(ba[!zr]|qux)baz.txt", "batbaz.txt"));
    assert(match("?(ba*|qux)baz.txt", "batbaz.txt"));
    assert(match("?(ba*|qux)baz.txt", "batttbaz.txt"));
    assert(match("?(ba*|qux)baz.txt", "quxbaz.txt"));
    assert(match("?(ba?(z|r)|qux)baz.txt", "bazbaz.txt"));
    assert(match("?(ba?(z|?(r))|qux)baz.txt", "bazbaz.txt"));
    assert(!match("?(foo|bar)baz.txt", "foobarbaz.txt"));
    assert(!match("?(ba[zr]|qux)baz.txt", "bazquxbaz.txt"));
    assert(!match("?(ba[!zr]|qux)baz.txt", "bazbaz.txt"));
  },
});

Deno.test({
  name: "[path] globToRegExp() *(pattern-list) (extended: match zero or more)",
  fn(): void {
    assert(match("*(foo).txt", "foo.txt"));
    assert(!match("*(foo).txt", "foo.txt", { extended: false }));
    assert(match("*(foo).txt", "bar(foo).txt", { extended: false }));
    assert(match("*(foo).txt", "foofoo.txt"));
    assert(match("*(foo).txt", ".txt"));
    assert(match("*(fooo).txt", ".txt"));
    assert(!match("*(fooo).txt", "foo.txt"));
    assert(match("*(foo|bar).txt", "foobar.txt"));
    assert(match("*(foo|bar).txt", "barbar.txt"));
    assert(match("*(foo|bar).txt", "barfoobar.txt"));
    assert(match("*(foo|bar).txt", ".txt"));
    assert(match("*(foo|ba[rt]).txt", "bat.txt"));
    assert(match("*(foo|b*[rt]).txt", "blat.txt"));
    assert(!match("*(foo|b*[rt]).txt", "tlat.txt"));
    assert(match("*(*).txt", "whatever.txt"));
    assert(match("*(foo|bar)/**/*.txt", "foo/hello/world/bar.txt"));
    assert(match("*(foo|bar)/**/*.txt", "foo/world/bar.txt"));
  },
});

Deno.test({
  name: "[path] globToRegExp() +(pattern-list) (extended: match 1 or more)",
  fn(): void {
    assert(match("+(foo).txt", "foo.txt"));
    assert(!match("+(foo).txt", "foo.txt", { extended: false }));
    assert(match("+(foo).txt", "+(foo).txt", { extended: false }));
    assert(!match("+(foo).txt", ".txt"));
    assert(match("+(foo|bar).txt", "foobar.txt"));
  },
});

Deno.test({
  name: "[path] globToRegExp() @(pattern-list) (extended: match one)",
  fn(): void {
    assert(match("@(foo).txt", "foo.txt"));
    assert(!match("@(foo).txt", "foo.txt", { extended: false }));
    assert(match("@(foo).txt", "@(foo).txt", { extended: false }));
    assert(match("@(foo|baz)bar.txt", "foobar.txt"));
    assert(!match("@(foo|baz)bar.txt", "foobazbar.txt"));
    assert(!match("@(foo|baz)bar.txt", "foofoobar.txt"));
    assert(!match("@(foo|baz)bar.txt", "toofoobar.txt"));
  },
});

Deno.test({
  name: "[path] globToRegExp() !(pattern-list) (extended: match any except)",
  fn(): void {
    assert(match("!(boo).txt", "foo.txt"));
    assert(!match("!(boo).txt", "foo.txt", { extended: false }));
    assert(match("!(boo).txt", "!(boo).txt", { extended: false }));
    assert(match("!(foo|baz)bar.txt", "buzbar.txt"));
    assert(match("!({foo,bar})baz.txt", "notbaz.txt"));
    assert(!match("!({foo,bar})baz.txt", "foobaz.txt"));
  },
});

Deno.test({
  name:
    "[path] globToRegExp() Special extended characters should match themselves",
  fn(): void {
    const glob = "\\/$^+.()=!|,.*";
    assert(match(glob, glob));
    assert(match(glob, glob, { extended: false }));
  },
});

Deno.test({
  name: "[path] globToRegExp() Special extended characters in range",
  fn(): void {
    assertEquals(globToRegExp("[?*+@!|]", { os: "linux" }), /^[?*+@!|]\/*$/);
    assertEquals(globToRegExp("[!?*+@!|]", { os: "linux" }), /^[^?*+@!|]\/*$/);
  },
});

Deno.test({
  name: "[path] globToRegExp() Special RegExp characters in range",
  fn(): void {
    // Excluding characters checked in the previous test.
    assertEquals(globToRegExp("[\\$^.=]", { os: "linux" }), /^[\\$^.=]\/*$/);
    assertEquals(globToRegExp("[!\\$^.=]", { os: "linux" }), /^[^\\$^.=]\/*$/);
    assertEquals(globToRegExp("[^^]", { os: "linux" }), /^[\^^]\/*$/);
  },
});

Deno.test({
  name: "[path] globToRegExp() Repeating separators",
  fn() {
    assert(match("foo/bar", "foo//bar"));
    assert(match("foo//bar", "foo/bar"));
    assert(match("foo//bar", "foo//bar"));
    assert(match("**/bar", "foo//bar"));
    assert(match("**//bar", "foo/bar"));
    assert(match("**//bar", "foo//bar"));
  },
});

Deno.test({
  name: "[path] globToRegExp() Trailing separators",
  fn() {
    assert(match("foo", "foo/"));
    assert(match("foo/", "foo"));
    assert(match("foo/", "foo/"));
    assert(match("**", "foo/"));
    assert(match("**/", "foo"));
    assert(match("**/", "foo/"));
  },
});

Deno.test({
  name: "[path] globToRegExp() Backslashes on Windows",
  fn() {
    assert(match("foo/bar", "foo\\bar", { os: "windows" }));
    assert(match("foo\\bar", "foo/bar", { os: "windows" }));
    assert(match("foo\\bar", "foo\\bar", { os: "windows" }));
    assert(match("**/bar", "foo\\bar", { os: "windows" }));
    assert(match("**\\bar", "foo/bar", { os: "windows" }));
    assert(match("**\\bar", "foo\\bar", { os: "windows" }));
  },
});

Deno.test({
  name: "[path] GlobToRegExpOptions::extended",
  fn() {
    const pattern1 = globToRegExp("?(foo|bar)");
    assertEquals("foo".match(pattern1)?.[0], "foo");
    assertEquals("bar".match(pattern1)?.[0], "bar");

    const pattern2 = globToRegExp("?(foo|bar)", { extended: false });
    assertEquals("foo".match(pattern2)?.[0], undefined);
    assertEquals("bar".match(pattern2)?.[0], undefined);
    assertEquals("?(foo|bar)".match(pattern2)?.[0], "?(foo|bar)");
  },
});

Deno.test({
  name: "[path] GlobToRegExpOptions::globstar",
  fn() {
    const pattern1 = globToRegExp("**/foo");
    assertEquals("foo".match(pattern1)?.[0], "foo");
    assertEquals("path/to/foo".match(pattern1)?.[0], "path/to/foo");

    const pattern2 = globToRegExp("**/foo", { globstar: false });
    assertEquals("foo".match(pattern2)?.[0], undefined);
    assertEquals("path/to/foo".match(pattern2)?.[0], undefined);
    assertEquals("path-to/foo".match(pattern2)?.[0], "path-to/foo");
  },
});

Deno.test({
  name: "[path] GlobToRegExpOptions::os",
  fn() {
    const pattern1 = globToRegExp("foo/bar", { os: "linux" });
    assertEquals("foo/bar".match(pattern1)?.[0], "foo/bar");
    assertEquals("foo\\bar".match(pattern1)?.[0], undefined);

    const pattern2 = globToRegExp("foo/bar", { os: "windows" });
    assertEquals("foo/bar".match(pattern2)?.[0], "foo/bar");
    assertEquals("foo\\bar".match(pattern2)?.[0], "foo\\bar");
  },
});

Deno.test({
  name: "[path] isGlob()",
  fn(): void {
    // should be true if valid glob pattern
    assert(isGlob("!foo.js"));
    assert(isGlob("*.js"));
    assert(isGlob("!*.js"));
    assert(isGlob("!foo"));
    assert(isGlob("!foo.js"));
    assert(isGlob("**/abc.js"));
    assert(isGlob("abc/*.js"));
    assert(isGlob("@.(?:abc)"));
    assert(isGlob("@.(?!abc)"));

    // should be false if invalid glob pattern
    assert(!isGlob(""));
    assert(!isGlob("~/abc"));
    assert(!isGlob("~/abc"));
    assert(!isGlob("~/(abc)"));
    assert(!isGlob("+~(abc)"));
    assert(!isGlob("."));
    assert(!isGlob("@.(abc)"));
    assert(!isGlob("aa"));
    assert(!isGlob("who?"));
    assert(!isGlob("why!?"));
    assert(!isGlob("where???"));
    assert(!isGlob("abc!/def/!ghi.js"));
    assert(!isGlob("abc.js"));
    assert(!isGlob("abc/def/!ghi.js"));
    assert(!isGlob("abc/def/ghi.js"));

    // Should be true if path has regex capture group
    assert(isGlob("abc/(?!foo).js"));
    assert(isGlob("abc/(?:foo).js"));
    assert(isGlob("abc/(?=foo).js"));
    assert(isGlob("abc/(a|b).js"));
    assert(isGlob("abc/(a|b|c).js"));
    assert(isGlob("abc/(foo bar)/*.js"));

    // Should be false if the path has parens but is not a valid capture group
    assert(!isGlob("abc/(?foo).js"));
    assert(!isGlob("abc/(a b c).js"));
    assert(!isGlob("abc/(ab).js"));
    assert(!isGlob("abc/(abc).js"));
    assert(!isGlob("abc/(foo bar).js"));

    // should be false if the capture group is imbalanced
    assert(!isGlob("abc/(?ab.js"));
    assert(!isGlob("abc/(ab.js"));
    assert(!isGlob("abc/(a|b.js"));
    assert(!isGlob("abc/(a|b|c.js"));

    // should be true if the path has a regex character class
    assert(isGlob("abc/[abc].js"));
    assert(isGlob("abc/[^abc].js"));
    assert(isGlob("abc/[1-3].js"));

    // should be false if the character class is not balanced
    assert(!isGlob("abc/[abc.js"));
    assert(!isGlob("abc/[^abc.js"));
    assert(!isGlob("abc/[1-3.js"));

    // should be false if the character class is escaped
    assert(!isGlob("abc/\\[abc].js"));
    assert(!isGlob("abc/\\[^abc].js"));
    assert(!isGlob("abc/\\[1-3].js"));

    // should be true if the path has brace characters
    assert(isGlob("abc/{a,b}.js"));
    assert(isGlob("abc/{a..z}.js"));
    assert(isGlob("abc/{a..z..2}.js"));

    // should be false if (basic) braces are not balanced
    assert(!isGlob("abc/\\{a,b}.js"));
    assert(!isGlob("abc/\\{a..z}.js"));
    assert(!isGlob("abc/\\{a..z..2}.js"));

    // should be true if the path has regex characters
    assert(isGlob("!&(abc)"));
    assert(isGlob("!*.js"));
    assert(isGlob("!foo"));
    assert(isGlob("!foo.js"));
    assert(isGlob("**/abc.js"));
    assert(isGlob("*.js"));
    assert(isGlob("*z(abc)"));
    assert(isGlob("[1-10].js"));
    assert(isGlob("[^abc].js"));
    assert(isGlob("[a-j]*[^c]b/c"));
    assert(isGlob("[abc].js"));
    assert(isGlob("a/b/c/[a-z].js"));
    assert(isGlob("abc/(aaa|bbb).js"));
    assert(isGlob("abc/*.js"));
    assert(isGlob("abc/{a,b}.js"));
    assert(isGlob("abc/{a..z..2}.js"));
    assert(isGlob("abc/{a..z}.js"));

    assert(!isGlob("$(abc)"));
    assert(!isGlob("&(abc)"));
    assert(!isGlob("Who?.js"));
    assert(!isGlob("? (abc)"));
    assert(!isGlob("?.js"));
    assert(!isGlob("abc/?.js"));

    // should be false if regex characters are escaped
    assert(!isGlob("\\?.js"));
    assert(!isGlob("\\[1-10\\].js"));
    assert(!isGlob("\\[^abc\\].js"));
    assert(!isGlob("\\[a-j\\]\\*\\[^c\\]b/c"));
    assert(!isGlob("\\[abc\\].js"));
    assert(!isGlob("\\a/b/c/\\[a-z\\].js"));
    assert(!isGlob("abc/\\(aaa|bbb).js"));
    assert(!isGlob("abc/\\?.js"));
  },
});

Deno.test("[path] normalizeGlob() Globstar", function (): void {
  assertEquals(normalizeGlob(`**${SEP}..`, { globstar: true }), `**${SEP}..`);
});

Deno.test("[path] joinGlobs() Globstar", function (): void {
  assertEquals(joinGlobs(["**", ".."], { globstar: true }), `**${SEP}..`);
});
