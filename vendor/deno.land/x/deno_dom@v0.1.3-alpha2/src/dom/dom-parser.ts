import { getLock, setLock } from "../constructor-lock.ts";
import { nodesFromString } from "../deserialize.ts";
import { HTMLDocument, DocumentType } from "./document.ts";
import type { Element } from "./element.ts";

export type DOMParserMimeType =
  "text/html"
  | "text/xml"
  | "application/xml"
  | "application/xhtml+xml"
  | "image/svg+xml";

export class DOMParser {
  parseFromString(source: string, mimeType: DOMParserMimeType): HTMLDocument | null {
    if (mimeType !== "text/html") {
      throw new Error(`DOMParser: "${ mimeType }" unimplemented`); // TODO
    }

    setLock(false);
    const doc = new HTMLDocument();

    setLock(false);
    const docType = new DocumentType("html", "", "");
    doc.appendChild(docType);

    const fakeDoc = nodesFromString(source);
    let htmlNode: Element | null = null;

    for (const child of fakeDoc.childNodes) {
      doc.appendChild(child);

      if (child.nodeName === "HTML") {
        htmlNode = <Element> child;
      }
    }

    setLock(true);

    if (htmlNode) {
      for (const child of htmlNode.childNodes) {
        switch ((<Element> child).tagName) {
          case "HEAD":
            doc.head = <Element> child;
            break;
          case "BODY":
            doc.body = <Element> child;
            break;
        }
      }
    }

    return doc;
  }
}

