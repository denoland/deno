// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { parseMediaType } from "./mod.ts";

Deno.test({
  name: "media_types - parseMediaType()",
  fn() {
    const nameFoo = { "name": "foo" };
    const fixtures: [string, string, Record<string, string> | undefined][] = [
      [`form-data; name="foo"`, "form-data", nameFoo],
      [` form-data ; name=foo`, "form-data", nameFoo],
      [`FORM-DATA;name="foo"`, "form-data", nameFoo],
      [` FORM-DATA ; name="foo"`, "form-data", nameFoo],
      [` FORM-DATA ; name="foo"`, "form-data", nameFoo],
      [`form-data; key=value;  blah="value";name="foo" `, "form-data", {
        key: "value",
        blah: "value",
        name: "foo",
      }],
      [
        `application/x-stuff; title*=us-ascii'en-us'This%20is%20%2A%2A%2Afun%2A%2A%2A`,
        "application/x-stuff",
        {
          title: "This is ***fun***",
        },
      ],
      [
        `message/external-body; access-type=URL; ` +
        `URL*0="ftp://";` +
        `URL*1="cs.utk.edu/pub/moore/bulk-mailer/bulk-mailer.tar"`,
        "message/external-body",
        {
          "access-type": "URL",
          url: "ftp://cs.utk.edu/pub/moore/bulk-mailer/bulk-mailer.tar",
        },
      ],
      [
        `application/x-stuff; ` +
        `title*0*=us-ascii'en'This%20is%20even%20more%20; ` +
        `title*1*=%2A%2A%2Afun%2A%2A%2A%20; ` +
        `title*2="isn't it!"`,
        `application/x-stuff`,
        {
          title: "This is even more ***fun*** isn't it!",
        },
      ],
      [`attachment`, "attachment", undefined],
      [`ATTACHMENT`, "attachment", undefined],
      [`attachment; filename="foo.html"`, "attachment", {
        filename: "foo.html",
      }],
      [`attachment; filename="f\\oo.html"`, "attachment", {
        filename: "f\\oo.html",
      }],
      [`attachment; filename="Here's a semicolon;.html"`, "attachment", {
        filename: "Here's a semicolon;.html",
      }],
      [`attachment; filename="foo-%c3%a4-%e2%82%ac.html"`, "attachment", {
        filename: "foo-%c3%a4-%e2%82%ac.html",
      }],
      [
        `attachment; filename*=''foo-%c3%a4-%e2%82%ac.html`,
        "attachment",
        undefined,
      ],
      [`attachment; filename*=UTF-8''foo-a%cc%88.html`, "attachment", {
        filename: "foo-ä.html",
      }],
      [`attachment; filename*0="foo."; filename*1="html"`, "attachment", {
        filename: "foo.html",
      }],
      [`form-data; firstname="Брэд"; lastname="Фицпатрик"`, "form-data", {
        firstname: "Брэд",
        lastname: "Фицпатрик",
      }],
      [
        `form-data; name="file"; filename="C:\\dev\\go\\robots.txt"`,
        "form-data",
        { name: "file", filename: `C:\\dev\\go\\robots.txt` },
      ],
      [
        `form-data; name="file"; filename="C:\\新建文件夹\\中文第二次测试.mp4"`,
        "form-data",
        { name: "file", filename: `C:\\新建文件夹\\中文第二次测试.mp4` },
      ],
    ];

    for (const [fixture, mediaType, params] of fixtures) {
      assertEquals(parseMediaType(fixture), [mediaType, params]);
    }
  },
});
