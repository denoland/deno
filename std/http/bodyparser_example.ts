// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.


import { serve, Response } from "./../http/server.ts";
import { FormFieldData, BodyParser } from "./bodyparser.ts";


(async (): Promise<void> => {
  const server = serve("127.0.0.1:3001");
  const headers = new Headers();
  for await (const req of server) {
    let body: string;
    const contentType = req.headers.get('Content-Type');
    if (req.method === "POST") {
      const httpBody: Uint8Array = await req.body();
      const bodyParser = new BodyParser(contentType, httpBody);
      // const dataList: BodyFormData[] = await parseFormUrlencoded(httpBody);
      const dataList: FormFieldData[] = await bodyParser.getFormData();
      body = JSON.stringify(dataList);
      // const httpBodyStr = new TextDecoder().decode(httpBody);
      // body = JSON.stringify({
      //   contentType: req.headers.get('Content-Type'),
      //   body: httpBodyStr,
      // })
      // Deno.writeFileSync(`./${dataList[1].filename}`, dataList[1].value as Uint8Array)
      body = JSON.stringify(dataList);
    } else {
      body = `
          <form action="/" method="POST">
            <input name="a" value="001" />
            <input name="b" value="002" />
            <input name="c" value="003" />
            <button type="submit">submit</button>
          </form>
          `;
      if (req.url === '/multipart') {
        body = `
          <form action="/" method="POST" enctype="multipart/form-data" >
            <input name="a" value="001"/>
            <input name="b" type="file"/>
            <button type="submit">submit</button>
          </form>
          `;
      }
      headers.set("Content-Type", "text/html");
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
