// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-explicit-any no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/**
 * Iterator for the URLSearchParams class, used to iterate over key-value pairs in search parameters.
 *
 * @example
 * ```ts
 * const url = new URL('https://example.org/path?a=1&b=2');
 * const queryString = url.search.substring(1); // Remove the leading '?'
 * const params = new URLSearchParams(queryString);
 * const iterator = params.entries();
 * console.log(iterator.next().value); // ['a', '1']
 * console.log(iterator.next().value); // ['b', '2']
 * ```
 *
 * @category URL
 */
interface URLSearchParamsIterator<T>
  extends IteratorObject<T, BuiltinIteratorReturn, unknown> {
  [Symbol.iterator](): URLSearchParamsIterator<T>;
}

/**
 * URLSearchParams provides methods for working with the query string of a URL.
 *
 * Use this interface to:
 * - Parse query parameters from URLs
 * - Build and modify query strings
 * - Handle form data (when used with FormData)
 * - Safely encode/decode URL parameter values
 *
 * @category URL
 */
interface URLSearchParams {
  /** Appends a specified key/value pair as a new search parameter.
   *
   * ```ts
   * let searchParams = new URLSearchParams();
   * searchParams.append('name', 'first');
   * searchParams.append('name', 'second');
   * ```
   */
  append(name: string, value: string): void;

  /** Deletes search parameters that match a name, and optional value,
   * from the list of all search parameters.
   *
   * ```ts
   * let searchParams = new URLSearchParams([['name', 'value']]);
   * searchParams.delete('name');
   * searchParams.delete('name', 'value');
   * ```
   */
  delete(name: string, value?: string): void;

  /** Returns all the values associated with a given search parameter
   * as an array.
   *
   * ```ts
   * searchParams.getAll('name');
   * ```
   */
  getAll(name: string): string[];

  /** Returns the first value associated to the given search parameter.
   *
   * ```ts
   * searchParams.get('name');
   * ```
   */
  get(name: string): string | null;

  /** Returns a boolean value indicating if a given parameter,
   * or parameter and value pair, exists.
   *
   * ```ts
   * searchParams.has('name');
   * searchParams.has('name', 'value');
   * ```
   */
  has(name: string, value?: string): boolean;

  /** Sets the value associated with a given search parameter to the
   * given value. If there were several matching values, this method
   * deletes the others. If the search parameter doesn't exist, this
   * method creates it.
   *
   * ```ts
   * searchParams.set('name', 'value');
   * ```
   */
  set(name: string, value: string): void;

  /** Sort all key/value pairs contained in this object in place and
   * return undefined. The sort order is according to Unicode code
   * points of the keys.
   *
   * ```ts
   * searchParams.sort();
   * ```
   */
  sort(): void;

  /** Calls a function for each element contained in this object in
   * place and return undefined. Optionally accepts an object to use
   * as this when executing callback as second argument.
   *
   * ```ts
   * const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   * params.forEach((value, key, parent) => {
   *   console.log(value, key, parent);
   * });
   * ```
   */
  forEach(
    callbackfn: (value: string, key: string, parent: this) => void,
    thisArg?: any,
  ): void;

  /** Returns an iterator allowing to go through all keys contained
   * in this object.
   *
   * ```ts
   * const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   * for (const key of params.keys()) {
   *   console.log(key);
   * }
   * ```
   */
  keys(): URLSearchParamsIterator<string>;

  /** Returns an iterator allowing to go through all values contained
   * in this object.
   *
   * ```ts
   * const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   * for (const value of params.values()) {
   *   console.log(value);
   * }
   * ```
   */
  values(): URLSearchParamsIterator<string>;

  /** Returns an iterator allowing to go through all key/value
   * pairs contained in this object.
   *
   * ```ts
   * const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   * for (const [key, value] of params.entries()) {
   *   console.log(key, value);
   * }
   * ```
   */
  entries(): URLSearchParamsIterator<[string, string]>;

  /** Returns an iterator allowing to go through all key/value
   * pairs contained in this object.
   *
   * ```ts
   * const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   * for (const [key, value] of params) {
   *   console.log(key, value);
   * }
   * ```
   */
  [Symbol.iterator](): URLSearchParamsIterator<[string, string]>;

  /** Returns a query string suitable for use in a URL.
   *
   * ```ts
   * searchParams.toString();
   * ```
   */
  toString(): string;

  /** Contains the number of search parameters
   *
   * ```ts
   * searchParams.size
   * ```
   */
  readonly size: number;
}

/** @category URL */
declare var URLSearchParams: {
  readonly prototype: URLSearchParams;
  /**
   * Creates a new URLSearchParams object for parsing query strings.
   *
   * URLSearchParams is Deno's built-in query string parser, providing a standard
   * way to parse, manipulate, and stringify URL query parameters. Instead of manually
   * parsing query strings with regex or string operations, use this API for robust
   * handling of URL query parameters.
   *
   * @example
   * ```ts
   * // From a URL object's query string (recommended approach for parsing query strings in URLs)
   * const url = new URL('https://example.org/path?foo=bar&baz=qux');
   * const params = url.searchParams;  // No need to manually extract the query string
   * console.log(params.get('foo'));  // Logs "bar"
   *
   * // Manually parsing a query string from a URL
   * const urlString = 'https://example.org/path?foo=bar&baz=qux';
   * const queryString = urlString.split('?')[1];  // Extract query string part
   * const params2 = new URLSearchParams(queryString);
   * console.log(params2.get('foo'));  // Logs "bar"
   *
   * // Empty search parameters
   * const params3 = new URLSearchParams();
   * console.log(params3.toString());  // Logs ""
   *
   * // From a string
   * const params4 = new URLSearchParams("foo=bar&baz=qux");
   * console.log(params4.get("foo"));  // Logs "bar"
   *
   * // From an array of pairs
   * const params5 = new URLSearchParams([["foo", "1"], ["bar", "2"]]);
   * console.log(params5.toString());  // Logs "foo=1&bar=2"
   *
   * // From a record object
   * const params6 = new URLSearchParams({"foo": "1", "bar": "2"});
   * console.log(params6.toString());  // Logs "foo=1&bar=2"
   * ```
   */
  new (
    init?:
      | Iterable<string[]>
      | Record<string, string>
      | string
      | URLSearchParams,
  ): URLSearchParams;
};

/** The URL interface represents an object providing static methods used for
 * creating, parsing, and manipulating URLs in Deno.
 *
 * Use the URL API for safely parsing, constructing, normalizing, and encoding URLs.
 * This is the preferred way to work with URLs in Deno rather than manual string
 * manipulation which can lead to errors and security issues.
 *
 * @see https://developer.mozilla.org/docs/Web/API/URL
 *
 * @category URL
 */
interface URL {
  /**
   * The hash property of the URL interface is a string that starts with a `#` and is followed by the fragment identifier of the URL.
   * It returns an empty string if the URL does not contain a fragment identifier.
   *
   * @example
   * ```ts
   * const myURL = new URL('https://example.org/foo#bar');
   * console.log(myURL.hash);  // Logs "#bar"
   *
   * const myOtherURL = new URL('https://example.org');
   * console.log(myOtherURL.hash);  // Logs ""
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/hash
   */
  hash: string;

  /**
   * The `host` property of the URL interface is a string that includes the {@linkcode URL.hostname} and the {@linkcode URL.port} if one is specified in the URL includes by including a `:` followed by the port number.
   *
   * @example
   * ```ts
   * const myURL = new URL('https://example.org/foo');
   * console.log(myURL.host);  // Logs "example.org"
   *
   * const myOtherURL = new URL('https://example.org:8080/foo');
   * console.log(myOtherURL.host);  // Logs "example.org:8080"
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/host
   */
  host: string;

  /**
   * The `hostname` property of the URL interface is a string that represents the fully qualified domain name of the URL.
   *
   * @example
   * ```ts
   * const myURL = new URL('https://foo.example.org/bar');
   * console.log(myURL.hostname);  // Logs "foo.example.org"
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/hostname
   */
  hostname: string;

  /**
   * The `href` property of the URL interface is a string that represents the complete URL.
   *
   * @example
   * ```ts
   * const myURL = new URL('https://foo.example.org/bar?baz=qux#quux');
   * console.log(myURL.href);  // Logs "https://foo.example.org/bar?baz=qux#quux"
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/href
   */
  href: string;

  /**
   * The `toString()` method of the URL interface returns a string containing the complete URL.
   *
   * @example
   * ```ts
   * const myURL = new URL('https://foo.example.org/bar');
   * console.log(myURL.toString());  // Logs "https://foo.example.org/bar"
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/toString
   */
  toString(): string;

  /**
   * The `origin` property of the URL interface is a string that represents the origin of the URL, that is the {@linkcode URL.protocol}, {@linkcode URL.host}, and {@linkcode URL.port}.
   *
   * @example
   * ```ts
   * const myURL = new URL('https://foo.example.org/bar');
   * console.log(myURL.origin);  // Logs "https://foo.example.org"
   *
   * const myOtherURL = new URL('https://example.org:8080/foo');
   * console.log(myOtherURL.origin);  // Logs "https://example.org:8080"
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/origin
   */
  readonly origin: string;

  /**
   * The `password` property of the URL interface is a string that represents the password specified in the URL.
   *
   * @example
   * ```ts
   * const myURL = new URL('https://someone:somepassword@example.org/baz');
   * console.log(myURL.password);  // Logs "somepassword"
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/password
   */
  password: string;

  /**
   * The `pathname` property of the URL interface is a string that represents the path of the URL.
   *
   * @example
   * ```ts
   * const myURL = new URL('https://example.org/foo/bar');
   * console.log(myURL.pathname);  // Logs "/foo/bar"
   *
   * const myOtherURL = new URL('https://example.org');
   * console.log(myOtherURL.pathname);  // Logs "/"
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/pathname
   */
  pathname: string;

  /**
   * The `port` property of the URL interface is a string that represents the port of the URL if an explicit port has been specified in the URL.
   *
   * @example
   * ```ts
   * const myURL = new URL('https://example.org:8080/foo');
   * console.log(myURL.port);  // Logs "8080"
   *
   * const myOtherURL = new URL('https://example.org/foo');
   * console.log(myOtherURL.port);  // Logs ""
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/port
   */
  port: string;

  /**
   * The `protocol` property of the URL interface is a string that represents the protocol scheme of the URL and includes a trailing `:`.
   *
   * @example
   * ```ts
   * const myURL = new URL('https://example.org/foo');
   * console.log(myURL.protocol);  // Logs "https:"
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/protocol
   */
  protocol: string;

  /**
   * The `search` property of the URL interface is a string that represents the search string, or the query string, of the URL.
   * This includes the `?` character and the but excludes identifiers within the represented resource such as the {@linkcode URL.hash}. More granular control can be found using {@linkcode URL.searchParams} property.
   *
   * @example
   * ```ts
   * const myURL = new URL('https://example.org/foo?bar=baz');
   * console.log(myURL.search);  // Logs "?bar=baz"
   *
   * const myOtherURL = new URL('https://example.org/foo?bar=baz#quux');
   * console.log(myOtherURL.search);  // Logs "?bar=baz"
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/search
   */
  search: string;

  /**
   * The `searchParams` property of the URL interface provides a direct interface to
   * query parameters through a {@linkcode URLSearchParams} object.
   *
   * This property offers a convenient way to:
   * - Parse URL query parameters
   * - Manipulate query strings
   * - Add, modify, or delete URL parameters
   * - Work with form data in a URL-encoded format
   * - Handle query string encoding/decoding automatically
   *
   * @example
   * ```ts
   * // Parse and access query parameters from a URL
   * const myURL = new URL('https://example.org/search?term=deno&page=2&sort=desc');
   * const params = myURL.searchParams;
   *
   * console.log(params.get('term'));  // Logs "deno"
   * console.log(params.get('page'));  // Logs "2"
   *
   * // Check if a parameter exists
   * console.log(params.has('sort'));  // Logs true
   *
   * // Add or modify parameters (automatically updates the URL)
   * params.append('filter', 'recent');
   * params.set('page', '3');
   * console.log(myURL.href);  // URL is updated with new parameters
   *
   * // Remove a parameter
   * params.delete('sort');
   *
   * // Iterate over all parameters
   * for (const [key, value] of params) {
   *   console.log(`${key}: ${value}`);
   * }
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/searchParams
   */
  readonly searchParams: URLSearchParams;

  /**
   * The `username` property of the URL interface is a string that represents the username of the URL.
   *
   * @example
   * ```ts
   * const myURL = new URL('https://someone:somepassword@example.org/baz');
   * console.log(myURL.username);  // Logs "someone"
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/username
   */
  username: string;

  /**
   * The `toJSON()` method of the URL interface returns a JSON representation of the URL.
   *
   * @example
   * ```ts
   * const myURL = new URL('https://example.org/foo');
   * console.log(myURL.toJSON());   // Logs "https://example.org/foo"
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/toJSON
   */
  toJSON(): string;
}

/** The URL interface represents an object providing static methods used for
 * creating, parsing, and manipulating URLs.
 *
 * @see https://developer.mozilla.org/docs/Web/API/URL
 *
 * @category URL
 */
declare var URL: {
  readonly prototype: URL;
  /**
   * Creates a new URL object by parsing the specified URL string with an optional base URL.
   * Throws a TypeError If the URL is invalid or if a relative URL is provided without a base.
   *
   * Use this to parse and validate URLs safely. Use this instead of string
   * manipulation to ensure correct URL handling, proper encoding, and protection against
   * security issues like path traversal attacks.
   *
   * @example
   * ```ts
   * // Creating a URL from an absolute URL string
   * const url1 = new URL('https://example.org/foo');
   * console.log(url1.href);  // Logs "https://example.org/foo"
   *
   * // Creating a URL from a relative URL string with a base URL
   * const url2 = new URL('/bar', 'https://example.org');
   * console.log(url2.href);  // Logs "https://example.org/bar"
   *
   * // Joining path segments safely (prevents path traversal)
   * const baseUrl = 'https://api.example.com/v1';
   * const userInput = '../secrets'; // Potentially malicious input
   * const safeUrl = new URL(userInput, baseUrl);
   * console.log(safeUrl.href); // Correctly resolves to "https://api.example.com/secrets"
   *
   * // Constructing URLs with proper encoding
   * const search = 'query with spaces';
   * const url3 = new URL('https://example.org/search');
   * url3.searchParams.set('q', search); // Automatically handles URL encoding
   * console.log(url3.href); // "https://example.org/search?q=query+with+spaces"
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/URL
   */
  new (url: string | URL, base?: string | URL): URL;

  /**
   * Parses a URL string or URL object and returns a URL object.
   *
   * @example
   * ```ts
   * const myURL = URL.parse('https://example.org');
   * console.log(myURL.href);  // Logs "https://example.org/"
   * console.log(myURL.hostname);  // Logs "example.org"
   * console.log(myURL.pathname);  // Logs "/"
   * console.log(myURL.protocol);  // Logs "https:"
   *
   * const baseURL = new URL('https://example.org');
   * const myNewURL = URL.parse('/foo', baseURL);
   * console.log(myNewURL.href);  // Logs "https://example.org/foo"
   * console.log(myNewURL.hostname);  // Logs "example.org"
   * console.log(myNewURL.pathname);  // Logs "/foo"
   * console.log(myNewURL.protocol);  // Logs "https:"
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/parse_static
   */
  parse(url: string | URL, base?: string | URL): URL | null;

  /**
   * Returns a boolean value indicating if a URL string is valid and can be parsed.
   *
   * @example
   * ```ts
   * // Check if an absolute URL string is valid
   * console.log(URL.canParse('https://example.org'));  // Logs true
   * console.log(URL.canParse('https:://example.org'));  // Logs false
   *
   * // Check if a relative URL string with a base is valid
   * console.log(URL.canParse('/foo', 'https://example.org'));  // Logs true
   * console.log(URL.canParse('/foo', 'https:://example.org'));  // Logs false
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/canParse_static
   */
  canParse(url: string | URL, base?: string | URL): boolean;

  /**
   * Creates a unique, temporary URL that represents a given Blob, File, or MediaSource object.
   *
   * This method is particularly useful for:
   * - Creating URLs for dynamically generated content
   * - Working with blobs in a browser context
   * - Creating workers from dynamically generated code
   * - Setting up temporary URL references for file downloads
   *
   * Note: Always call URL.revokeObjectURL() when you're done using the URL to prevent memory leaks.
   *
   * @example
   * ```ts
   * // Create a URL string for a Blob
   * const blob = new Blob(["Hello, world!"], { type: "text/plain" });
   * const url = URL.createObjectURL(blob);
   * console.log(url);  // Logs something like "blob:null/1234-5678-9101-1121"
   *
   * // Dynamic web worker creation in Deno
   * const workerCode = `
   *   self.onmessage = (e) => {
   *     self.postMessage(e.data.toUpperCase());
   *   };
   * `;
   * const workerBlob = new Blob([workerCode], { type: "application/javascript" });
   * const workerUrl = URL.createObjectURL(workerBlob);
   * const worker = new Worker(workerUrl, { type: "module" });
   *
   * worker.onmessage = (e) => console.log(e.data);
   * worker.postMessage("hello from deno");
   *
   * // Always revoke when done to prevent memory leaks
   * URL.revokeObjectURL(workerUrl);
   * ```
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/createObjectURL_static
   */
  createObjectURL(blob: Blob): string;

  /**
   * Revokes a previously created object URL, freeing the memory associated with it.
   *
   * Important for memory management in applications that create dynamic URLs.
   * Once an object URL is revoked:
   * - It can no longer be used to fetch the content it referenced
   * - The browser/runtime is allowed to release the memory or resources associated with it
   * - Workers created via the URL will continue to run, but the URL becomes invalid for new creations
   *
   * For security and performance in Deno applications, always revoke object URLs as soon as
   * they're no longer needed, especially when processing large files or generating many URLs.
   *
   * @see https://developer.mozilla.org/docs/Web/API/URL/revokeObjectURL_static
   */
  revokeObjectURL(url: string): void;
};

/** @category URL */
interface URLPatternInit {
  protocol?: string;
  username?: string;
  password?: string;
  hostname?: string;
  port?: string;
  pathname?: string;
  search?: string;
  hash?: string;
  baseURL?: string;
}

/** @category URL */
type URLPatternInput = string | URLPatternInit;

/** @category URL */
interface URLPatternComponentResult {
  input: string;
  groups: Record<string, string | undefined>;
}

/** `URLPatternResult` is the object returned from `URLPattern.exec`.
 *
 * @category URL
 */
interface URLPatternResult {
  /** The inputs provided when matching. */
  inputs: [URLPatternInit] | [URLPatternInit, string];

  /** The matched result for the `protocol` matcher. */
  protocol: URLPatternComponentResult;
  /** The matched result for the `username` matcher. */
  username: URLPatternComponentResult;
  /** The matched result for the `password` matcher. */
  password: URLPatternComponentResult;
  /** The matched result for the `hostname` matcher. */
  hostname: URLPatternComponentResult;
  /** The matched result for the `port` matcher. */
  port: URLPatternComponentResult;
  /** The matched result for the `pathname` matcher. */
  pathname: URLPatternComponentResult;
  /** The matched result for the `search` matcher. */
  search: URLPatternComponentResult;
  /** The matched result for the `hash` matcher. */
  hash: URLPatternComponentResult;
}

/**
 * Options for the {@linkcode URLPattern} constructor.
 *
 * @category URL
 */
interface URLPatternOptions {
  /**
   * Enables case-insensitive matching.
   *
   * @default {false}
   */
  ignoreCase: boolean;
}

/**
 * The URLPattern API provides a web platform primitive for matching URLs based
 * on a convenient pattern syntax.
 *
 * Common use cases for URLPattern include:
 * - Building routers for web applications
 * - Pattern-matching URLs for middleware
 * - Extracting parameters from URL paths
 * - URL-based feature toggles
 * - Routing in serverless and edge functions
 *
 * The syntax is based on path-to-regexp, supporting wildcards, named capture groups,
 * regular groups, and group modifiers - similar to Express.js route patterns.
 *
 * @example
 * ```ts
 * // Basic routing with URLPattern (similar to Express.js)
 * const routes = [
 *   new URLPattern({ pathname: "/users" }),
 *   new URLPattern({ pathname: "/users/:id" }),
 *   new URLPattern({ pathname: "/products/:category/:id?" }),
 * ];
 *
 * // Check incoming request against routes
 * function handleRequest(req: Request) {
 *   const url = new URL(req.url);
 *
 *   for (const route of routes) {
 *     const match = route.exec(url);
 *     if (match) {
 *       // Extract parameters from the URL
 *       const params = match.pathname.groups;
 *       return new Response(`Matched: ${JSON.stringify(params)}`);
 *     }
 *   }
 *
 *   return new Response("Not found", { status: 404 });
 * }
 * ```
 *
 * @example
 * ```ts
 * // Matching different URL parts
 * const apiPattern = new URLPattern({
 *   protocol: "https",
 *   hostname: "api.example.com",
 *   pathname: "/v:version/:resource/:id?",
 *   search: "*", // Match any query string
 * });
 *
 * const match = apiPattern.exec("https://api.example.com/v1/users/123?format=json");
 * if (match) {
 *   console.log(match.pathname.groups.version); // "1"
 *   console.log(match.pathname.groups.resource); // "users"
 *   console.log(match.pathname.groups.id); // "123"
 * }
 * ```
 *
 * @category URL
 */
interface URLPattern {
  /**
   * Test if the given input matches the stored pattern.
   *
   * The input can either be provided as an absolute URL string with an optional base,
   * relative URL string with a required base, or as individual components
   * in the form of an `URLPatternInit` object.
   *
   * ```ts
   * const pattern = new URLPattern("https://example.com/books/:id");
   *
   * // Test an absolute url string.
   * console.log(pattern.test("https://example.com/books/123")); // true
   *
   * // Test a relative url with a base.
   * console.log(pattern.test("/books/123", "https://example.com")); // true
   *
   * // Test an object of url components.
   * console.log(pattern.test({ pathname: "/books/123" })); // true
   * ```
   */
  test(input: URLPatternInput, baseURL?: string): boolean;

  /**
   * Match the given input against the stored pattern.
   *
   * The input can either be provided as an absolute URL string with an optional base,
   * relative URL string with a required base, or as individual components
   * in the form of an `URLPatternInit` object.
   *
   * ```ts
   * const pattern = new URLPattern("https://example.com/books/:id");
   *
   * // Match an absolute url string.
   * let match = pattern.exec("https://example.com/books/123");
   * console.log(match.pathname.groups.id); // 123
   *
   * // Match a relative url with a base.
   * match = pattern.exec("/books/123", "https://example.com");
   * console.log(match.pathname.groups.id); // 123
   *
   * // Match an object of url components.
   * match = pattern.exec({ pathname: "/books/123" });
   * console.log(match.pathname.groups.id); // 123
   * ```
   */
  exec(input: URLPatternInput, baseURL?: string): URLPatternResult | null;

  /** The pattern string for the `protocol`. */
  readonly protocol: string;
  /** The pattern string for the `username`. */
  readonly username: string;
  /** The pattern string for the `password`. */
  readonly password: string;
  /** The pattern string for the `hostname`. */
  readonly hostname: string;
  /** The pattern string for the `port`. */
  readonly port: string;
  /** The pattern string for the `pathname`. */
  readonly pathname: string;
  /** The pattern string for the `search`. */
  readonly search: string;
  /** The pattern string for the `hash`. */
  readonly hash: string;

  /** Whether or not any of the specified groups use regexp groups. */
  readonly hasRegExpGroups: boolean;
}

/**
 * The URLPattern API provides a web platform primitive for matching URLs based
 * on a convenient pattern syntax.
 *
 * The syntax is based on path-to-regexp. Wildcards, named capture groups,
 * regular groups, and group modifiers are all supported.
 *
 * ```ts
 * // Specify the pattern as structured data.
 * const pattern = new URLPattern({ pathname: "/users/:user" });
 * const match = pattern.exec("https://blog.example.com/users/joe");
 * console.log(match.pathname.groups.user); // joe
 * ```
 *
 * ```ts
 * // Specify a fully qualified string pattern.
 * const pattern = new URLPattern("https://example.com/books/:id");
 * console.log(pattern.test("https://example.com/books/123")); // true
 * console.log(pattern.test("https://deno.land/books/123")); // false
 * ```
 *
 * ```ts
 * // Specify a relative string pattern with a base URL.
 * const pattern = new URLPattern("/article/:id", "https://blog.example.com");
 * console.log(pattern.test("https://blog.example.com/article")); // false
 * console.log(pattern.test("https://blog.example.com/article/123")); // true
 * ```
 *
 * @category URL
 */
declare var URLPattern: {
  readonly prototype: URLPattern;
  new (
    input: URLPatternInput,
    baseURL: string,
    options?: URLPatternOptions,
  ): URLPattern;
  new (input?: URLPatternInput, options?: URLPatternOptions): URLPattern;
};
