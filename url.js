/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/
 * Forked from https://github.com/github/url-polyfill
 * Version 16c1aa Feb 9 2018.
 */

(function(scope) {
  "use strict";

  // feature detect for URL constructor
  var hasWorkingUrl = false;
  if (!scope.forceJURL) {
    try {
      var u = new URL("b", "http://a");
      u.pathname = "c%20d";
      hasWorkingUrl = u.href === "http://a/c%20d";
    } catch (e) {}
  }

  if (hasWorkingUrl) return;

  var relative = Object.create(null);
  relative["ftp"] = 21;
  relative["file"] = 0;
  relative["gopher"] = 70;
  relative["http"] = 80;
  relative["https"] = 443;
  relative["ws"] = 80;
  relative["wss"] = 443;

  var relativePathDotMapping = Object.create(null);
  relativePathDotMapping["%2e"] = ".";
  relativePathDotMapping[".%2e"] = "..";
  relativePathDotMapping["%2e."] = "..";
  relativePathDotMapping["%2e%2e"] = "..";

  function isRelativeScheme(scheme) {
    return relative[scheme] !== undefined;
  }

  function invalid() {
    clear.call(this);
    this._isInvalid = true;
  }

  function IDNAToASCII(h) {
    if ("" == h) {
      invalid.call(this);
    }
    // XXX
    return h.toLowerCase();
  }

  function percentEscape(c) {
    var unicode = c.charCodeAt(0);
    if (
      unicode > 0x20 &&
      unicode < 0x7f &&
      // " # < > ? `
      [0x22, 0x23, 0x3c, 0x3e, 0x3f, 0x60].indexOf(unicode) == -1
    ) {
      return c;
    }
    return encodeURIComponent(c);
  }

  function percentEscapeQuery(c) {
    // XXX This actually needs to encode c using encoding and then
    // convert the bytes one-by-one.

    var unicode = c.charCodeAt(0);
    if (
      unicode > 0x20 &&
      unicode < 0x7f &&
      // " # < > ` (do not escape '?')
      [0x22, 0x23, 0x3c, 0x3e, 0x60].indexOf(unicode) == -1
    ) {
      return c;
    }
    return encodeURIComponent(c);
  }

  var EOF = undefined,
    ALPHA = /[a-zA-Z]/,
    ALPHANUMERIC = /[a-zA-Z0-9\+\-\.]/;

  function parse(input, stateOverride, base) {
    function err(message) {
      errors.push(message);
    }

    var state = stateOverride || "scheme start",
      cursor = 0,
      buffer = "",
      seenAt = false,
      seenBracket = false,
      errors = [];

    loop: while (
      (input[cursor - 1] != EOF || cursor == 0) &&
      !this._isInvalid
    ) {
      var c = input[cursor];
      switch (state) {
        case "scheme start":
          if (c && ALPHA.test(c)) {
            buffer += c.toLowerCase(); // ASCII-safe
            state = "scheme";
          } else if (!stateOverride) {
            buffer = "";
            state = "no scheme";
            continue;
          } else {
            err("Invalid scheme.");
            break loop;
          }
          break;

        case "scheme":
          if (c && ALPHANUMERIC.test(c)) {
            buffer += c.toLowerCase(); // ASCII-safe
          } else if (":" == c) {
            this._scheme = buffer;
            buffer = "";
            if (stateOverride) {
              break loop;
            }
            if (isRelativeScheme(this._scheme)) {
              this._isRelative = true;
            }
            if ("file" == this._scheme) {
              state = "relative";
            } else if (
              this._isRelative &&
              base &&
              base._scheme == this._scheme
            ) {
              state = "relative or authority";
            } else if (this._isRelative) {
              state = "authority first slash";
            } else {
              state = "scheme data";
            }
          } else if (!stateOverride) {
            buffer = "";
            cursor = 0;
            state = "no scheme";
            continue;
          } else if (EOF == c) {
            break loop;
          } else {
            err("Code point not allowed in scheme: " + c);
            break loop;
          }
          break;

        case "scheme data":
          if ("?" == c) {
            query = "?";
            state = "query";
          } else if ("#" == c) {
            this._fragment = "#";
            state = "fragment";
          } else {
            // XXX error handling
            if (EOF != c && "\t" != c && "\n" != c && "\r" != c) {
              this._schemeData += percentEscape(c);
            }
          }
          break;

        case "no scheme":
          if (!base || !isRelativeScheme(base._scheme)) {
            err("Missing scheme.");
            invalid.call(this);
          } else {
            state = "relative";
            continue;
          }
          break;

        case "relative or authority":
          if ("/" == c && "/" == input[cursor + 1]) {
            state = "authority ignore slashes";
          } else {
            err("Expected /, got: " + c);
            state = "relative";
            continue;
          }
          break;

        case "relative":
          this._isRelative = true;
          if ("file" != this._scheme) this._scheme = base._scheme;
          if (EOF == c) {
            this._host = base._host;
            this._port = base._port;
            this._path = base._path.slice();
            this._query = base._query;
            this._username = base._username;
            this._password = base._password;
            break loop;
          } else if ("/" == c || "\\" == c) {
            if ("\\" == c) err("\\ is an invalid code point.");
            state = "relative slash";
          } else if ("?" == c) {
            this._host = base._host;
            this._port = base._port;
            this._path = base._path.slice();
            this._query = "?";
            this._username = base._username;
            this._password = base._password;
            state = "query";
          } else if ("#" == c) {
            this._host = base._host;
            this._port = base._port;
            this._path = base._path.slice();
            this._query = base._query;
            this._fragment = "#";
            this._username = base._username;
            this._password = base._password;
            state = "fragment";
          } else {
            var nextC = input[cursor + 1];
            var nextNextC = input[cursor + 2];
            if (
              "file" != this._scheme ||
              !ALPHA.test(c) ||
              (nextC != ":" && nextC != "|") ||
              (EOF != nextNextC &&
                "/" != nextNextC &&
                "\\" != nextNextC &&
                "?" != nextNextC &&
                "#" != nextNextC)
            ) {
              this._host = base._host;
              this._port = base._port;
              this._username = base._username;
              this._password = base._password;
              this._path = base._path.slice();
              this._path.pop();
            }
            state = "relative path";
            continue;
          }
          break;

        case "relative slash":
          if ("/" == c || "\\" == c) {
            if ("\\" == c) {
              err("\\ is an invalid code point.");
            }
            if ("file" == this._scheme) {
              state = "file host";
            } else {
              state = "authority ignore slashes";
            }
          } else {
            if ("file" != this._scheme) {
              this._host = base._host;
              this._port = base._port;
              this._username = base._username;
              this._password = base._password;
            }
            state = "relative path";
            continue;
          }
          break;

        case "authority first slash":
          if ("/" == c) {
            state = "authority second slash";
          } else {
            err("Expected '/', got: " + c);
            state = "authority ignore slashes";
            continue;
          }
          break;

        case "authority second slash":
          state = "authority ignore slashes";
          if ("/" != c) {
            err("Expected '/', got: " + c);
            continue;
          }
          break;

        case "authority ignore slashes":
          if ("/" != c && "\\" != c) {
            state = "authority";
            continue;
          } else {
            err("Expected authority, got: " + c);
          }
          break;

        case "authority":
          if ("@" == c) {
            if (seenAt) {
              err("@ already seen.");
              buffer += "%40";
            }
            seenAt = true;
            for (var i = 0; i < buffer.length; i++) {
              var cp = buffer[i];
              if ("\t" == cp || "\n" == cp || "\r" == cp) {
                err("Invalid whitespace in authority.");
                continue;
              }
              // XXX check URL code points
              if (":" == cp && null === this._password) {
                this._password = "";
                continue;
              }
              var tempC = percentEscape(cp);
              null !== this._password
                ? (this._password += tempC)
                : (this._username += tempC);
            }
            buffer = "";
          } else if (
            EOF == c ||
            "/" == c ||
            "\\" == c ||
            "?" == c ||
            "#" == c
          ) {
            cursor -= buffer.length;
            buffer = "";
            state = "host";
            continue;
          } else {
            buffer += c;
          }
          break;

        case "file host":
          if (EOF == c || "/" == c || "\\" == c || "?" == c || "#" == c) {
            if (
              buffer.length == 2 &&
              ALPHA.test(buffer[0]) &&
              (buffer[1] == ":" || buffer[1] == "|")
            ) {
              state = "relative path";
            } else if (buffer.length == 0) {
              state = "relative path start";
            } else {
              this._host = IDNAToASCII.call(this, buffer);
              buffer = "";
              state = "relative path start";
            }
            continue;
          } else if ("\t" == c || "\n" == c || "\r" == c) {
            err("Invalid whitespace in file host.");
          } else {
            buffer += c;
          }
          break;

        case "host":
        case "hostname":
          if (":" == c && !seenBracket) {
            // XXX host parsing
            this._host = IDNAToASCII.call(this, buffer);
            buffer = "";
            state = "port";
            if ("hostname" == stateOverride) {
              break loop;
            }
          } else if (
            EOF == c ||
            "/" == c ||
            "\\" == c ||
            "?" == c ||
            "#" == c
          ) {
            this._host = IDNAToASCII.call(this, buffer);
            buffer = "";
            state = "relative path start";
            if (stateOverride) {
              break loop;
            }
            continue;
          } else if ("\t" != c && "\n" != c && "\r" != c) {
            if ("[" == c) {
              seenBracket = true;
            } else if ("]" == c) {
              seenBracket = false;
            }
            buffer += c;
          } else {
            err("Invalid code point in host/hostname: " + c);
          }
          break;

        case "port":
          if (/[0-9]/.test(c)) {
            buffer += c;
          } else if (
            EOF == c ||
            "/" == c ||
            "\\" == c ||
            "?" == c ||
            "#" == c ||
            stateOverride
          ) {
            if ("" != buffer) {
              var temp = parseInt(buffer, 10);
              if (temp != relative[this._scheme]) {
                this._port = temp + "";
              }
              buffer = "";
            }
            if (stateOverride) {
              break loop;
            }
            state = "relative path start";
            continue;
          } else if ("\t" == c || "\n" == c || "\r" == c) {
            err("Invalid code point in port: " + c);
          } else {
            invalid.call(this);
          }
          break;

        case "relative path start":
          if ("\\" == c) err("'\\' not allowed in path.");
          state = "relative path";
          if ("/" != c && "\\" != c) {
            continue;
          }
          break;

        case "relative path":
          if (
            EOF == c ||
            "/" == c ||
            "\\" == c ||
            (!stateOverride && ("?" == c || "#" == c))
          ) {
            if ("\\" == c) {
              err("\\ not allowed in relative path.");
            }
            var tmp;
            if ((tmp = relativePathDotMapping[buffer.toLowerCase()])) {
              buffer = tmp;
            }
            if (".." == buffer) {
              this._path.pop();
              if ("/" != c && "\\" != c) {
                this._path.push("");
              }
            } else if ("." == buffer && "/" != c && "\\" != c) {
              this._path.push("");
            } else if ("." != buffer) {
              if (
                "file" == this._scheme &&
                this._path.length == 0 &&
                buffer.length == 2 &&
                ALPHA.test(buffer[0]) &&
                buffer[1] == "|"
              ) {
                buffer = buffer[0] + ":";
              }
              this._path.push(buffer);
            }
            buffer = "";
            if ("?" == c) {
              this._query = "?";
              state = "query";
            } else if ("#" == c) {
              this._fragment = "#";
              state = "fragment";
            }
          } else if ("\t" != c && "\n" != c && "\r" != c) {
            buffer += percentEscape(c);
          }
          break;

        case "query":
          if (!stateOverride && "#" == c) {
            this._fragment = "#";
            state = "fragment";
          } else if (EOF != c && "\t" != c && "\n" != c && "\r" != c) {
            this._query += percentEscapeQuery(c);
          }
          break;

        case "fragment":
          if (EOF != c && "\t" != c && "\n" != c && "\r" != c) {
            this._fragment += c;
          }
          break;
      }

      cursor++;
    }
  }

  function clear() {
    this._scheme = "";
    this._schemeData = "";
    this._username = "";
    this._password = null;
    this._host = "";
    this._port = "";
    this._path = [];
    this._query = "";
    this._fragment = "";
    this._isInvalid = false;
    this._isRelative = false;
  }

  // Does not process domain names or IP addresses.
  // Does not handle encoding for the query parameter.
  function jURL(url, base /* , encoding */) {
    if (base !== undefined && !(base instanceof jURL))
      base = new jURL(String(base));

    url = String(url);

    this._url = url;
    clear.call(this);

    var input = url.replace(/^[ \t\r\n\f]+|[ \t\r\n\f]+$/g, "");
    // encoding = encoding || 'utf-8'

    parse.call(this, input, null, base);
  }

  jURL.prototype = {
    toString: function() {
      return this.href;
    },
    get href() {
      if (this._isInvalid) return this._url;

      var authority = "";
      if ("" != this._username || null != this._password) {
        authority =
          this._username +
          (null != this._password ? ":" + this._password : "") +
          "@";
      }

      return (
        this.protocol +
        (this._isRelative ? "//" + authority + this.host : "") +
        this.pathname +
        this._query +
        this._fragment
      );
    },
    set href(href) {
      clear.call(this);
      parse.call(this, href);
    },

    get protocol() {
      return this._scheme + ":";
    },
    set protocol(protocol) {
      if (this._isInvalid) return;
      parse.call(this, protocol + ":", "scheme start");
    },

    get host() {
      return this._isInvalid
        ? ""
        : this._port
          ? this._host + ":" + this._port
          : this._host;
    },
    set host(host) {
      if (this._isInvalid || !this._isRelative) return;
      parse.call(this, host, "host");
    },

    get hostname() {
      return this._host;
    },
    set hostname(hostname) {
      if (this._isInvalid || !this._isRelative) return;
      parse.call(this, hostname, "hostname");
    },

    get port() {
      return this._port;
    },
    set port(port) {
      if (this._isInvalid || !this._isRelative) return;
      parse.call(this, port, "port");
    },

    get pathname() {
      return this._isInvalid
        ? ""
        : this._isRelative
          ? "/" + this._path.join("/")
          : this._schemeData;
    },
    set pathname(pathname) {
      if (this._isInvalid || !this._isRelative) return;
      this._path = [];
      parse.call(this, pathname, "relative path start");
    },

    get search() {
      return this._isInvalid || !this._query || "?" == this._query
        ? ""
        : this._query;
    },
    set search(search) {
      if (this._isInvalid || !this._isRelative) return;
      this._query = "?";
      if ("?" == search[0]) search = search.slice(1);
      parse.call(this, search, "query");
    },

    get hash() {
      return this._isInvalid || !this._fragment || "#" == this._fragment
        ? ""
        : this._fragment;
    },
    set hash(hash) {
      if (this._isInvalid) return;
      this._fragment = "#";
      if ("#" == hash[0]) hash = hash.slice(1);
      parse.call(this, hash, "fragment");
    },

    get origin() {
      var host;
      if (this._isInvalid || !this._scheme) {
        return "";
      }
      // javascript: Gecko returns String(""), WebKit/Blink String("null")
      // Gecko throws error for "data://"
      // data: Gecko returns "", Blink returns "data://", WebKit returns "null"
      // Gecko returns String("") for file: mailto:
      // WebKit/Blink returns String("SCHEME://") for file: mailto:
      switch (this._scheme) {
        case "data":
        case "file":
        case "javascript":
        case "mailto":
          return "null";
      }
      host = this.host;
      if (!host) {
        return "";
      }
      return this._scheme + "://" + host;
    }
  };

  // Copy over the static methods
  var OriginalURL = scope.URL;
  if (OriginalURL) {
    jURL.createObjectURL = function(blob) {
      // IE extension allows a second optional options argument.
      // http://msdn.microsoft.com/en-us/library/ie/hh772302(v=vs.85).aspx
      return OriginalURL.createObjectURL.apply(OriginalURL, arguments);
    };
    jURL.revokeObjectURL = function(url) {
      OriginalURL.revokeObjectURL(url);
    };
  }

  scope.URL = jURL;
})(window);
