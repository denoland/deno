// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { test } from "./../testing/mod.ts";
import { assert, assertEquals } from "./../testing/asserts.ts";
import { BufReader } from "./../io/bufio.ts";
import { TextProtoReader } from "./../textproto/mod.ts";

let fileServer: Deno.Process;

async function startFileServer(): Promise<void> {
  fileServer = Deno.run({
    args: [
      Deno.execPath(),
      "run",
      "--allow-read",
      "--allow-net",
      "http/bodyparser_server.ts",
      ".",
      "--cors"
    ],
    stdout: "piped"
  });
  // Once fileServer is ready it will write to its stdout.
  const r = new TextProtoReader(new BufReader(fileServer.stdout!));
  const s = await r.readLine();
  assert(s !== Deno.EOF && s.includes("server listening"));
}

function killFileServer(): void {
  fileServer.close();
  fileServer.stdout!.close();
}

test(async function parseFormUrlencoded(): Promise<void> {
  await startFileServer();
  try {
    const res = await fetch("http://localhost:4500/parseFormUrlencoded", {
      method: "POST",
      headers: {
        "Content-Type": "application/x-www-form-urlencoded"
      },
      redirect: "follow", // manual, *follow, error
      referrer: "no-referrer", // no-referrer, *client
      body: "a=001&b=002&c=003"
    });
    const expectData = [
      { name: "a", value: "001", type: "text" },
      { name: "b", value: "002", type: "text" },
      { name: "c", value: "003", type: "text" }
    ];
    const bodyData = await res.json();
    assertEquals(bodyData, expectData);
  } finally {
    killFileServer();
  }
});

test(async function bodyParserGetFormDataUrlencoded(): Promise<void> {
  await startFileServer();
  try {
    const res = await fetch(
      "http://localhost:4500/BodyParser/getFormData/urlencoded",
      {
        method: "POST",
        headers: {
          "Content-Type": "application/x-www-form-urlencoded"
        },
        redirect: "follow", // manual, *follow, error
        referrer: "no-referrer", // no-referrer, *client
        body: "a=001&b=002&c=003"
      }
    );
    const expectData = [
      { name: "a", value: "001", type: "text" },
      { name: "b", value: "002", type: "text" },
      { name: "c", value: "003", type: "text" }
    ];
    const bodyData = await res.json();
    assertEquals(bodyData, expectData);
  } finally {
    killFileServer();
  }
});

test(async function bodyParserGetFormDataMultipart(): Promise<void> {
  await startFileServer();
  const multipartBody = `------WebKitFormBoundaryF2FPVKMYJmaBhBnJ\r\nContent-Disposition: form-data; name="a"\r\n\r\n001\r\n------WebKitFormBoundaryF2FPVKMYJmaBhBnJ\r\nContent-Disposition: form-data; name="b"; filename="file.txt"\r\nContent-Type: text/plain\r\n\r\nhello world!\r\nhello deno!\r\n------WebKitFormBoundaryF2FPVKMYJmaBhBnJ--\r\n`;

  try {
    const res = await fetch(
      "http://localhost:4500/BodyParser/getFormData/multipart",
      {
        method: "POST",
        headers: {
          "Content-Type":
            "multipart/form-data; boundary=----WebKitFormBoundaryF2FPVKMYJmaBhBnJ"
        },
        redirect: "follow", // manual, *follow, error
        referrer: "no-referrer", // no-referrer, *client
        // body: new TextEncoder().encode(body),
        body: multipartBody
      }
    );
    const expectData = [
      { name: "a", value: "001", type: "text" },
      {
        name: "b",
        type: "file",
        filetype: "text/plain",
        filename: "file.txt",
        value: {
          "0": 104,
          "1": 101,
          "2": 108,
          "3": 108,
          "4": 111,
          "5": 32,
          "6": 119,
          "7": 111,
          "8": 114,
          "9": 108,
          "10": 100,
          "11": 33,
          "12": 13,
          "13": 10,
          "14": 104,
          "15": 101,
          "16": 108,
          "17": 108,
          "18": 111,
          "19": 32,
          "20": 100,
          "21": 101,
          "22": 110,
          "23": 111,
          "24": 33
        }
      }
    ];
    const bodyData = await res.json();
    assertEquals(bodyData, expectData);
  } finally {
    killFileServer();
  }
});
