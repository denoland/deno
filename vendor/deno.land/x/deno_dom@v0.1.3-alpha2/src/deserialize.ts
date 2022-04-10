import { parse, parseFrag } from "./parser.ts";
import { setLock } from "./constructor-lock.ts";
import { Node, NodeType, Text, Comment } from "./dom/node.ts";
import { Element } from "./dom/element.ts";

export function nodesFromString(html: string): Node {
  setLock(false);
  const parsed = JSON.parse(parse(html));
  const node = nodeFromArray(parsed, null);
  setLock(true);

  return node;
}

export function fragmentNodesFromString(html: string): Node {
  setLock(false);
  const parsed = JSON.parse(parseFrag(html));
  const node = nodeFromArray(parsed, null);
  setLock(true);

  return node;
}

function nodeFromArray(data: any, parentNode: Node | null): Node {
  // For reference only:
  // type node = [NodeType, nodeName, attributes, node[]]
  //             | [NodeType, characterData]
  const elm = new Element(data[1], parentNode, data[2]);
  const childNodes = elm._getChildNodesMutator();
  let childNode: Node;

  for (const child of data.slice(3)) {
    switch (child[0]) {
      case NodeType.TEXT_NODE:
        childNode = new Text(child[1]);
        childNode.parentNode = childNode.parentElement = <Element> elm;
        childNodes.push(childNode);
        break;

      case NodeType.COMMENT_NODE:
        childNode = new Comment(child[1]);
        childNode.parentNode = childNode.parentElement = <Element> elm;
        childNodes.push(childNode);
        break;

      case NodeType.DOCUMENT_NODE:
      case NodeType.ELEMENT_NODE:
        nodeFromArray(child, elm);
        break;
    }
  }

  return elm;
}

