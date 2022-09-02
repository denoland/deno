import { serve } from "https://deno.land/std@0.153.0/http/server.ts";
import ChartJs from "https://esm.sh/v92/chart.js@^2.9.4";
import {
  Rect2D,
  SvgCanvas,
  SvgCanvas2DGradient
} from "https://esm.sh/v92/red-agate-svg-canvas@0.5.0";
import results from "../../results.json" assert { type: "json" };

const opts = {
  type: "line",
  data: {
    labels: results.labels,
    datasets: [{
        label: 'JS',
        data: results.jsValues,
        fill: false,
        borderColor: 'red',
        tension: 0.1
      }, {
        label: 'Native',
        data: results.nativeValues,
        fill: false,
        borderColor: 'green',
        tension: 0.1
      }],
  },
  options: {
    devicePixelRatio: 1,
    animation: undefined,
    events: [],
    responsive: false,

  },
};

serve(() => {
  const ctx = new SvgCanvas();

  ctx.canvas = {
    width: 800,
    height: 400,
    style: {
      width: "800px",
      height: "400px",
    },
  };

  ctx.fontHeightRatio = 2;

  const el = { getContext: () => ctx };

  const savedGradient = globalThis.CanvasGradient;
  globalThis.CanvasGradient = SvgCanvas2DGradient;

  try {
    new ChartJs.Chart(el, opts);
  } finally {
    if (savedGradient) {
      globalThis.CanvasGradient = savedGradient;
    }
  }

  const svgString = ctx.render(new Rect2D(0, 0, 800, 400), "px");

  return new Response(svgString, {
    headers: {
      "content-type": "image/svg+xml"
    }
  })
});