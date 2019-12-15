// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { serve, Response } from "./../http/server.ts";
import { parseFormUrlencoded, BodyParser, FormFieldData } from "./bodyparser.ts";


(async () => {
  const server = serve("127.0.0.1:4500");
  console.log("server listening");

  for await (const req of server) {
    let body: string = "404: Not Found!";
    const headers = new Headers();
    const contentType = req.headers.get("Content-Type");
    if (req.method === "POST") {
      if (req.url === '/parseFormUrlencoded') {
        const httpBody: Uint8Array = await req.body();
        const dataList: FormFieldData[] = await parseFormUrlencoded(httpBody);
        body = JSON.stringify(dataList);
      } else if (req.url === '/BodyParser/getFormData/urlencoded') {
        const httpBody: Uint8Array = await req.body();
        const bodyParser = new BodyParser(contentType, httpBody);
        const dataList: FormFieldData[] = await bodyParser.getFormData();
        body = JSON.stringify(dataList);
      } else if (req.url === '/BodyParser/getFormData/multipart') {
        const httpBody: Uint8Array = await req.body();
        const bodyParser = new BodyParser(contentType, httpBody);
        const dataList: FormFieldData[] = await bodyParser.getFormData();
        body = JSON.stringify(dataList);
      }
    }
    const stream: Uint8Array = new TextEncoder().encode(body);

    headers.set("Content-Length", `${stream.length}`);
    const res: Response = {
      status: 200,
      headers,
      body: stream,
    }
    await req.respond(res);
  }

})()
