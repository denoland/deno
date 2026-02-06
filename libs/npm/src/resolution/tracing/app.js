// Copyright 2018-2026 the Deno authors. MIT license.
// @ts-check

/**
 * @typedef {Object} TraceGraphSnapshot
 * @property {Record<string, number>} roots
 * @property {TraceNode[]} nodes
 * @property {TraceGraphPath} path
 */

/**
 * @typedef {Object} TraceNode
 * @property {number} id
 * @property {string} resolvedId
 * @property {Record<string, number>} children
 * @property {TraceNodeDependency[]} dependencies
 */

/**
 * @typedef {Object} TraceGraphPath
 * @property {string} specifier
 * @property {number} nodeId
 * @property {string} nv
 * @property {?TraceGraphPath} previous
 */

/**
 * @typedef {Object} TraceNodeDependency
 * @property {string} kind
 * @property {string} bareSpecifier
 * @property {string} name
 * @property {string} versionReq
 * @property {string | undefined} peerDepVersionReq
 */

/**
 * @typedef {Object} GraphNode
 * @property {TraceNode} rawNode
 * @property {number} id
 * @property {GraphNode[]} sources
 * @property {GraphNode[]} targets
 * @property {number} x
 * @property {number} y
 */

// @ts-types="npm:@types/d3@7.4"
import * as d3 from "https://cdn.jsdelivr.net/npm/d3@7/+esm";

/**
 * @type {TraceGraphSnapshot[]}
 */
// @ts-ignore rawTraces is defined in the generated html file.
const traces = rawTraces;
/** @type {Map<number, {x: number, y: number}>} */
const nodePositions = new Map();
const { nodeDepths, depthYChildCount } = analyzeTracesDepth();
/** @type {d3.ZoomTransform} */
let transform;
const graphDiv = document.getElementById("graph");
const infoDiv = document.getElementById("info");
const stepTextDiv = document.getElementById("stepText");
/** @type {GraphNode[]} */
let nodes = undefined;
initSlider(traces.length - 1, (index) => {
  refresh(index);
});

refresh(0);

/** @param {number} index */
function refresh(index) {
  const snapshot = traces[index];
  const snapshotNodesMap = new Map(snapshot.nodes.map((n) => [n.id, n]));

  const svg = d3.select("#graph svg");
  if (!svg.empty()) {
    // capture current zoom
    transform = d3.zoomTransform(
      /** @type {d3.ZoomedElementBaseType} */ (svg.node()),
    );
  }
  if (nodes) {
    // Save current node positions
    nodes.forEach((node) => {
      nodePositions.set(node.id, { x: node.x, y: node.y });
    });
  }

  graphDiv.replaceChildren(); // remove the children

  stepTextDiv.textContent = `${index + 1}/${traces.length}`;
  setInfoNode();
  createGraph();

  function createGraph() {
    const result = getNodesAndLinks();
    const { links, nodesMap } = result;
    nodes = result.nodes;
    const pathNodeIds = getPathNodeIds();

    const width = graphDiv.clientWidth;
    const height = graphDiv.clientHeight;
    const svg = d3
      .select("#graph")
      .append("svg")
      .attr("viewBox", [0, 0, width, height])
      .style("font", "40px sans-serif")
      .attr("width", width)
      .attr("height", height);

    const arrow = svg.append("svg:defs").selectAll("marker")
      .data(["end"])
      .enter().append("svg:marker")
      .attr("id", String)
      .attr("orient", "auto");
    const arrowInnerPath = arrow.append("svg:path").attr("fill", "#000");

    const drag = d3
      .drag()
      .on("drag", function (event, d) {
        d.x = event.x;
        d.y = event.y;
        d3.select(this).raise().attr("transform", `translate(${d.x}, ${d.y})`);
        refreshLinks();
      });

    const nodeRadius = 15;
    const linkThickness = 5;
    const linkG = svg.append("g");
    const link = linkG
      .selectAll("line")
      .data(links)
      .join("line")
      .attr("stroke-opacity", 0.6)
      .attr("stroke", (d) => {
        const bothOnPath = pathNodeIds.get(d.target) === d.source;
        return bothOnPath ? "red" : "black";
      })
      .style("stroke-width", linkThickness)
      .attr("marker-end", "url(#end)");
    link.append("title")
      .text((d) => {
        return d.specifier;
      });

    const nodeG = svg.append("g");
    const nodeGInner = nodeG.append("g")
      .selectAll("g")
      .data(nodes)
      .join("g")
      .attr("transform", (d) => {
        return `translate(${d.x}, ${d.y})`;
      }).call(/** @type {any} */ (drag));
    const nodeCircle = nodeGInner
      .append("circle")
      .attr("r", nodeRadius)
      .attr("fill", (d) => {
        const isGraphPath = pathNodeIds.has(d.id);
        return isGraphPath ? "red" : "blue";
      })
      .attr("stroke", "#000")
      .attr("id", (d) => `node${d.id}`)
      .on("click", (_, _d) => {
        // show more info on click in the future
      });
    nodeGInner
      .append("text")
      .attr("x", 50)
      .attr("y", "0.31em")
      .text((d) => {
        return d.rawNode.resolvedId;
      })
      .clone(true).lower()
      .attr("fill", "none")
      .attr("stroke", "white")
      .attr("stroke-width", 3);

    /** @type {number} */
    let sqrtK;
    const zoom = d3.zoom().on("zoom", (e) => {
      applyTransform(e.transform);
    });

    /** @param {d3.ZoomTransform} transform  */
    function applyTransform(transform) {
      nodeG.attr("transform", transform.toString());
      sqrtK = Math.sqrt(transform.k);
      nodeCircle.attr("r", nodeRadius / sqrtK)
        .attr("stroke-width", 1 / sqrtK);

      linkG.attr("transform", transform.toString());
      link.style("stroke-width", linkThickness / sqrtK);

      arrow.attr("markerWidth", 5)
        .attr("markerHeight", 5)
        .attr("viewBox", `0 0 ${5 / sqrtK} ${5 / sqrtK}`)
        .attr("refX", 8 / sqrtK)
        .attr("refY", 2.5 / sqrtK);
      arrowInnerPath.attr(
        "d",
        `M 0 0 L ${5 / sqrtK} ${2.5 / sqrtK} L 0 ${5 / sqrtK} z`,
      );
    }

    svg.call(/** @type {any} */ (zoom)).call(
      /** @type {any} */ (zoom.transform),
      transform ?? d3.zoomIdentity,
    );

    refreshLinks();

    function refreshLinks() {
      link
        .attr("x1", (d) => nodesMap.get(d.source).x)
        .attr("y1", (d) => nodesMap.get(d.source).y)
        .attr("x2", (d) => nodesMap.get(d.target).x)
        .attr("y2", (d) => nodesMap.get(d.target).y);
    }
  }

  function getNodesAndLinks() {
    /** @param {number} id */
    function getNodeY(id) {
      const nodeDepth = nodeDepths.get(id);
      let depthY = 0;
      for (let i = 0; i < nodeDepth.y; i++) {
        const childCount = depthYChildCount.get(i) ?? 1;
        depthY += childCount * 50;
      }
      const jitter = (Math.random() - 0.5) * 70;
      return depthY + nodeDepth.x * 200 + jitter;
    }

    /** @param {number} id */
    function getNodeX(id) {
      const nodeDepth = nodeDepths.get(id);
      const center = width / 2;
      const childCount = depthYChildCount.get(nodeDepth.y) ?? 0;
      const jitter = (Math.random() - 0.5) * 70;
      return center + (nodeDepth.x - (childCount / 2)) * 255 + jitter;
    }

    const width = graphDiv.clientWidth;
    /** @type {GraphNode[]} */
    const nodes = [];
    /** @type {Set<number>} */
    const seen = new Set();
    const pendingNodes = Object.values(snapshot.roots);
    while (pendingNodes.length > 0) {
      const id = pendingNodes.shift();
      if (seen.has(id)) {
        continue;
      }
      seen.add(id);
      const savedPosition = nodePositions.get(id);
      const node = snapshotNodesMap.get(id);
      nodes.push({
        id: node.id,
        rawNode: node,
        sources: /** @type {GraphNode[]} */ ([]),
        targets: /** @type {GraphNode[]} */ ([]),
        x: savedPosition?.x ?? getNodeX(node.id),
        y: savedPosition?.y ?? getNodeY(node.id),
      });
      pendingNodes.push(...Object.values(node.children));
    }
    const nodesMap = new Map(nodes.map((n) => [n.rawNode.id, n]));
    /** @type {{ source: number; target: number; specifier: string; }[]} */
    const links = [];

    for (const node of nodes) {
      const rawNode = node.rawNode;
      for (const [specifier, child] of Object.entries(rawNode.children)) {
        addLink(specifier, node, getNodeById(child));
      }
    }

    return { nodes, nodesMap, links };

    /**
     * @param {string} specifier
     * @param {GraphNode} source
     * @param {GraphNode} target
     */
    function addLink(specifier, source, target) {
      source.targets.push(target);
      target.sources.push(source);
      links.push({
        specifier,
        source: source.id,
        target: target.id,
      });
    }

    /** @param {number} id */
    function getNodeById(id) {
      const node = nodesMap.get(id);
      if (node == null) {
        throw new Error(`Could not find node: ${id}`);
      }
      return node;
    }
  }

  function getPathNodeIds() {
    let currentPath = snapshot.path;
    /** @type {Map<number, number | undefined>} */
    const nodes = new Map();
    while (currentPath != null) {
      nodes.set(currentPath.nodeId, currentPath.previous?.nodeId);
      currentPath = currentPath.previous;
    }
    return nodes;
  }

  function setInfoNode() {
    let currentPath = snapshot.path;
    infoDiv.replaceChildren(); // clear
    while (currentPath != null) {
      const currentNode = snapshotNodesMap.get(currentPath.nodeId);
      infoDiv.appendChild(getRawNodeDiv(currentNode));
      currentPath = currentPath.previous;
    }
  }
}

/**
 * @param {number} max
 * @param {(value: number) => void} onChange */
function initSlider(max, onChange) {
  /** @type {HTMLInputElement} */
  const input = document.querySelector("#slider input");
  input.min = "0";
  input.max = max.toString();
  input.addEventListener("input", () => {
    onChange(input.valueAsNumber);
  });
  input.value = "0";
}

/** @param {TraceNode} rawNode */
function getRawNodeDiv(rawNode) {
  const div = document.createElement("div");
  const title = document.createElement("h3");
  title.textContent = `${rawNode.resolvedId} (${rawNode.id})`;
  div.appendChild(title);
  const ul = document.createElement("ul");
  for (const dep of rawNode.dependencies) {
    const li = document.createElement("li");
    let text = `${dep.kind} - ${dep.bareSpecifier} - ${dep.versionReq}`;
    if (dep.peerDepVersionReq != null) {
      text += ` - ${dep.peerDepVersionReq}`;
    }
    li.textContent = text;
    ul.appendChild(li);
  }
  div.appendChild(ul);
  return div;
}

function analyzeTracesDepth() {
  /** @type {Map<number, { x: number, y: number }>} */
  const nodeDepths = new Map();
  /** @type {Map<number, number>} */
  const depthYChildCount = new Map();
  /** @type {Map<number, TraceNode>} */
  let nodesMap = new Map();
  /** @type {Set<number>} */
  const seenNodes = new Set();

  for (const snapshot of traces) {
    seenNodes.clear();
    nodesMap = new Map(snapshot.nodes.map((n) => [n.id, n]));
    setDepthY(
      Object.values(snapshot.roots).map((start) => nodesMap.get(start)),
    );
  }

  // certain nodes might be disconnected... add those here
  for (const snapshot of traces) {
    setDepthY(snapshot.nodes.filter((n) => !nodeDepths.has(n.id)));
  }

  return {
    nodeDepths,
    depthYChildCount,
  };

  /** @param {TraceNode[]} firstNodes */
  function setDepthY(firstNodes) {
    /** @type {[TraceNode, number][]} */
    const nodesToAnalyze = firstNodes.map((node) => [node, 0]);

    while (nodesToAnalyze.length > 0) {
      const next = nodesToAnalyze.shift();
      const [node, depth] = next;
      if (seenNodes.has(node.id)) {
        continue;
      }
      seenNodes.add(node.id);
      if (!nodeDepths.has(node.id)) {
        const childIndex = depthYChildCount.get(depth) ?? 0;
        nodeDepths.set(node.id, {
          y: depth,
          x: childIndex,
        });
        depthYChildCount.set(depth, childIndex + 1);
      }
      for (const child of Object.values(node.children)) {
        nodesToAnalyze.push([nodesMap.get(child), depth + 1]);
      }
    }
  }
}
