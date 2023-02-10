// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 crypto-browserify. All rights reserved. MIT license.
// deno-lint-ignore-file no-var

import { Buffer } from "../../buffer.ts";
import { CipherBase } from "./cipher_base.js";
import {
  assert,
  assertEquals,
  assertThrows,
} from "../../../testing/asserts.ts";

Deno.test("basic version", function () {
  function Cipher() {
    CipherBase.call(this);
  }
  Cipher.prototype = Object.create(CipherBase.prototype, {
    constructor: {
      value: Cipher,
      enumerable: false,
      writable: true,
      configurable: true,
    },
  });
  Cipher.prototype._update = function (input) {
    assert(Buffer.isBuffer(input));
    return input;
  };
  Cipher.prototype._final = function () {
    // noop
  };
  var cipher = new Cipher();
  var utf8 = "abc123abcd";
  var update = cipher.update(utf8, "utf8", "base64") + cipher.final("base64");
  var string = (Buffer.from(update, "base64")).toString();
  assertEquals(utf8, string);
});

Deno.test("hash mode", function () {
  function Cipher() {
    CipherBase.call(this, "finalName");
    this._cache = [];
  }
  Cipher.prototype = Object.create(CipherBase.prototype, {
    constructor: {
      value: Cipher,
      enumerable: false,
      writable: true,
      configurable: true,
    },
  });
  Cipher.prototype._update = function (input) {
    assert(Buffer.isBuffer(input));
    this._cache.push(input);
  };
  Cipher.prototype._final = function () {
    return Buffer.concat(this._cache);
  };
  var cipher = new Cipher();
  var utf8 = "abc123abcd";
  var update = cipher.update(utf8, "utf8").finalName("base64");
  var string = (Buffer.from(update, "base64")).toString();

  assertEquals(utf8, string);
});

Deno.test("hash mode as stream", function () {
  function Cipher() {
    CipherBase.call(this, "finalName");
    this._cache = [];
  }
  Cipher.prototype = Object.create(CipherBase.prototype, {
    constructor: {
      value: Cipher,
      enumerable: false,
      writable: true,
      configurable: true,
    },
  });
  Cipher.prototype._update = function (input) {
    assert(Buffer.isBuffer(input));
    this._cache.push(input);
  };
  Cipher.prototype._final = function () {
    return Buffer.concat(this._cache);
  };
  var cipher = new Cipher();
  cipher.on("error", function (e) {
    assert(!e);
  });
  var utf8 = "abc123abcd";
  cipher.end(utf8, "utf8");
  var update = cipher.read().toString("base64");
  var string = (Buffer.from(update, "base64")).toString();

  assertEquals(utf8, string);
});

Deno.test("encodings", async function (t) {
  Cipher.prototype = Object.create(CipherBase.prototype, {
    constructor: {
      value: Cipher,
      enumerable: false,
      writable: true,
      configurable: true,
    },
  });
  function Cipher() {
    CipherBase.call(this);
  }
  Cipher.prototype._update = function (input) {
    return input;
  };
  Cipher.prototype._final = function () {
    // noop
  };
  await t.step("mix and match encoding", function () {
    var cipher = new Cipher();
    cipher.update("foo", "utf8", "utf8");
    assertThrows(function () {
      cipher.update("foo", "utf8", "base64");
    });
    cipher = new Cipher();
    cipher.update("foo", "utf8", "base64");
    cipher.update("foo", "utf8");
    cipher.final("base64");
  });
  await t.step("handle long uft8 plaintexts", function () {
    var txt =
      "ふっかつ　あきる　すぶり　はやい　つける　まゆげ　たんさん　みんぞく　ねほりはほり　せまい　たいまつばな　ひはん";

    var cipher = new Cipher();
    var decipher = new Cipher();
    var enc = decipher.update(
      cipher.update(txt, "utf8", "base64"),
      "base64",
      "utf8",
    );
    enc += decipher.update(cipher.final("base64"), "base64", "utf8");
    enc += decipher.final("utf8");

    assertEquals(txt, enc);
  });
});
