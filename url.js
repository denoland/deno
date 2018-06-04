/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/
 * Forked from https://github.com/github/url-polyfill
 * Version 16c1aa Feb 9 2018.
 */

(function(scope) {
  "use strict";

  const STATE_SCHEME_START = Symbol("state scheme start");
  const STATE_SCHEME = Symbol("state scheme");
  const STATE_SCHEME_DATA = Symbol("state scheme data");
  const STATE_NO_SCHEME = Symbol("state no scheme");
  const STATE_RELATIVE_OR_AUTHORITY = Symbol("state relative or authority");
  const STATE_RELATIVE = Symbol("state relative");
  const STATE_RELATIVE_SLASH = Symbol("state relative slash");
  const STATE_AUTHORITY_FIRST_SLASH = Symbol("state authority first slash");
  const STATE_AUTHORITY_SECOND_SLASH = Symbol("state authority second slash");
  const STATE_AUTHORITY_IGNORE_SLASHES = Symbol("state authority ignore slashes");
  const STATE_AUTHORITY = Symbol("state authority");
  const STATE_FILE_HOST = Symbol("state file host");
  const STATE_HOST = Symbol("state host");
  const STATE_HOSTNAME = Symbol("state hostname");
  const STATE_PORT = Symbol("state port");
  const STATE_RELATIVE_PATH_START = Symbol("state relative path start");
  const STATE_RELATIVE_PATH = Symbol("state relative path");
  const STATE_QUERY = Symbol("state query");
  const STATE_FRAGMENT = Symbol("state fragment");

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

    var state = stateOverride || STATE_SCHEME_START,
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
        case STATE_SCHEME_START:
          if (c && ALPHA.test(c)) {
            buffer += c.toLowerCase(); // ASCII-safe
            state = STATE_SCHEME;
          } else if (!stateOverride) {
            buffer = "";
            state = STATE_NO_SCHEME;
            continue;
          } else {
            err("Invalid scheme.");
            break loop;
          }
          break;

        case STATE_SCHEME:
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
              state = STATE_RELATIVE;
            } else if (
              this._isRelative &&
              base &&
              base._scheme == this._scheme
            ) {
              state = STATE_RELATIVE_OR_AUTHORITY;
            } else if (this._isRelative) {
              state = STATE_AUTHORITY_FIRST_SLASH;
            } else {
              state = STATE_SCHEME_DATA;
            }
          } else if (!stateOverride) {
            buffer = "";
            cursor = 0;
            state = STATE_NO_SCHEME;
            continue;
          } else if (EOF == c) {
            break loop;
          } else {
            err("Code point not allowed in scheme: " + c);
            break loop;
          }
          break;

        case STATE_SCHEME_DATA:
          if ("?" == c) {
            query = "?";
            state = STATE_QUERY;
          } else if ("#" == c) {
            this._fragment = "#";
            state = STATE_FRAGMENT;
          } else {
            // XXX error handling
            if (EOF != c && "\t" != c && "\n" != c && "\r" != c) {
              this._schemeData += percentEscape(c);
            }
          }
          break;

        case STATE_NO_SCHEME:
          if (!base || !isRelativeScheme(base._scheme)) {
            err("Missing scheme.");
            invalid.call(this);
          } else {
            state = STATE_RELATIVE;
            continue;
          }
          break;

        case STATE_RELATIVE_OR_AUTHORITY:
          if ("/" == c && "/" == input[cursor + 1]) {
            state = STATE_AUTHORITY_IGNORE_SLASHES;
          } else {
            err("Expected /, got: " + c);
            state = STATE_RELATIVE;
            continue;
          }
          break;

        case STATE_RELATIVE:
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
            state = STATE_RELATIVE_SLASH;
          } else if ("?" == c) {
            this._host = base._host;
            this._port = base._port;
            this._path = base._path.slice();
            this._query = "?";
            this._username = base._username;
            this._password = base._password;
            state = STATE_QUERY;
          } else if ("#" == c) {
            this._host = base._host;
            this._port = base._port;
            this._path = base._path.slice();
            this._query = base._query;
            this._fragment = "#";
            this._username = base._username;
            this._password = base._password;
            state = STATE_FRAGMENT;
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
            state = STATE_RELATIVE_PATH;
            continue;
          }
          break;

        case STATE_RELATIVE_SLASH:
          if ("/" == c || "\\" == c) {
            if ("\\" == c) {
              err("\\ is an invalid code point.");
            }
            if ("file" == this._scheme) {
              state = STATE_FILE_HOST;
            } else {
              state = STATE_AUTHORITY_IGNORE_SLASHES;
            }
          } else {
            if ("file" != this._scheme) {
              this._host = base._host;
              this._port = base._port;
              this._username = base._username;
              this._password = base._password;
            }
            state = STATE_RELATIVE_PATH;
            continue;
          }
          break;

        case STATE_AUTHORITY_FIRST_SLASH:
          if ("/" == c) {
            state = STATE_AUTHORITY_SECOND_SLASH;
          } else {
            err("Expected '/', got: " + c);
            state = STATE_AUTHORITY_IGNORE_SLASHES;
            continue;
          }
          break;

        case STATE_AUTHORITY_SECOND_SLASH:
          state = STATE_AUTHORITY_IGNORE_SLASHES;
          if ("/" != c) {
            err("Expected '/', got: " + c);
            continue;
          }
          break;

        case STATE_AUTHORITY_IGNORE_SLASHES:
          if ("/" != c && "\\" != c) {
            state = STATE_AUTHORITY;
            continue;
          } else {
            err("Expected authority, got: " + c);
          }
          break;

        case STATE_AUTHORITY:
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
            state = STATE_HOST;
            continue;
          } else {
            buffer += c;
          }
          break;

        case STATE_FILE_HOST:
          if (EOF == c || "/" == c || "\\" == c || "?" == c || "#" == c) {
            if (
              buffer.length == 2 &&
              ALPHA.test(buffer[0]) &&
              (buffer[1] == ":" || buffer[1] == "|")
            ) {
              state = STATE_RELATIVE_PATH;
            } else if (buffer.length == 0) {
              state = STATE_RELATIVE_PATH_START;
            } else {
              this._host = IDNAToASCII.call(this, buffer);
              buffer = "";
              state = STATE_RELATIVE_PATH_START;
            }
            continue;
          } else if ("\t" == c || "\n" == c || "\r" == c) {
            err("Invalid whitespace in file host.");
          } else {
            buffer += c;
          }
          break;

        case STATE_HOST:
        case STATE_HOSTNAME:
          if (":" == c && !seenBracket) {
            // XXX host parsing
            this._host = IDNAToASCII.call(this, buffer);
            buffer = "";
            state = STATE_PORT;
            if (STATE_HOSTNAME == stateOverride) {
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
            state = STATE_RELATIVE_PATH_START;
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

        case STATE_PORT:
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
            state = STATE_RELATIVE_PATH_START;
            continue;
          } else if ("\t" == c || "\n" == c || "\r" == c) {
            err("Invalid code point in port: " + c);
          } else {
            invalid.call(this);
          }
          break;

        case STATE_RELATIVE_PATH_START:
          if ("\\" == c) err("'\\' not allowed in path.");
          state = STATE_RELATIVE_PATH;
          if ("/" != c && "\\" != c) {
            continue;
          }
          break;

        case STATE_RELATIVE_PATH:
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
              state = STATE_QUERY;
            } else if ("#" == c) {
              this._fragment = "#";
              state = STATE_FRAGMENT;
            }
          } else if ("\t" != c && "\n" != c && "\r" != c) {
            buffer += percentEscape(c);
          }
          break;

        case STATE_QUERY:
          if (!stateOverride && "#" == c) {
            this._fragment = "#";
            state = STATE_FRAGMENT;
          } else if (EOF != c && "\t" != c && "\n" != c && "\r" != c) {
            this._query += percentEscapeQuery(c);
          }
          break;

        case STATE_FRAGMENT:
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
      parse.call(this, protocol + ":", STATE_SCHEME_START);
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
      parse.call(this, host, STATE_HOST);
    },

    get hostname() {
      return this._host;
    },
    set hostname(hostname) {
      if (this._isInvalid || !this._isRelative) return;
      parse.call(this, hostname, STATE_HOSTNAME);
    },

    get port() {
      return this._port;
    },
    set port(port) {
      if (this._isInvalid || !this._isRelative) return;
      parse.call(this, port, STATE_PORT);
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
      parse.call(this, pathname, STATE_RELATIVE_PATH_START);
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
      parse.call(this, search, STATE_QUERY);
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
      parse.call(this, hash, STATE_FRAGMENT);
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
