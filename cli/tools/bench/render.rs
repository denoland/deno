// Copyright 2018-2026 the Deno authors. MIT license.

//! Console rendering for `deno bench`.
//!
//! Each benchmark occupies a compact three-line block: the summary numbers
//! (avg, min/max range, iter/s and a couple of percentiles) on the left, and a
//! small vertical histogram of the sample distribution on the right. The
//! histogram is drawn across all three lines so it has real height rather than
//! being a flat sparkline, and its bins are clamped to the 99th percentile so a
//! lone slow outlier doesn't squash the shape into the first bar. Bars are
//! tinted by where they fall relative to the mean: faster-than-average cyan,
//! the mean bucket yellow, slower-than-average magenta.
//!
//! Groups additionally get a horizontal barplot comparing average times before
//! the textual "summary" block.

use super::BenchStats;
use super::mitata;
use crate::colors;

// Fixed column widths (in characters). The name column grows to fit the
// longest benchmark name; everything to its right is fixed so the numbers line
// up across benchmarks and across groups within a file.
const NAME_MIN: usize = 9;
// Wide column holding, across the three lines: avg, "(min ... max)", iter/s.
// Sized so "(999.9 ms ... 999.9 ms)" fits exactly.
const MAIN_W: usize = 21;
// Narrow column holding p75 (line 1) and p99 (line 2).
const SUB_W: usize = 9;
// Histogram width in buckets and height in text rows.
const HIST_W: usize = 22;
const HIST_H: usize = 3;
const GAP: &str = "  ";

// Eight levels of vertical block glyphs (U+2581 ..= U+2588), written as escapes
// so the source stays ASCII.
const BLOCKS: [char; 8] = [
  '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}',
  '\u{2587}', '\u{2588}',
];

// Seven horizontal partial blocks (U+258F 1/8 .. U+2589 7/8) used for the
// barplot; a full bar cell is U+2588.
const HBLOCKS: [char; 7] = [
  '\u{258F}', '\u{258E}', '\u{258D}', '\u{258C}', '\u{258B}', '\u{258A}',
  '\u{2589}',
];

// U+2026 HORIZONTAL ELLIPSIS.
const ELLIPSIS: &str = "\u{2026}";

/// Width of the benchmark-name column for a set of names.
pub fn name_width(names: &[&str]) -> usize {
  let mut width = NAME_MIN;
  for name in names {
    width = width.max(name.chars().count());
  }
  width
}

/// The column header row.
pub fn header(name_width: usize) -> String {
  format!(
    "{:<nw$}{GAP}{:>MAIN_W$}{GAP}{:>SUB_W$}{GAP}{}",
    "benchmark",
    format!("avg (min {ELLIPSIS} max)"),
    "p75 / p99",
    "distribution",
    nw = name_width,
  )
}

/// The dashed rule drawn under the header.
pub fn separator(name_width: usize) -> String {
  let width =
    name_width + GAP.len() + MAIN_W + GAP.len() + SUB_W + GAP.len() + HIST_W;
  "-".repeat(width)
}

/// A single successful benchmark result: a three-line block.
pub fn benchmark(name: &str, stats: &BenchStats, name_width: usize) -> String {
  let rows = histogram(stats);

  let avg =
    colors::yellow(format!("{:>MAIN_W$}", mitata::fmt_duration(stats.avg)));
  let range = padded_range(stats.min, stats.max);
  let iters =
    colors::gray(format!("{:>MAIN_W$}", mitata::avg_to_iter_per_s(stats.avg)));
  let p75 =
    colors::gray(format!("{:>SUB_W$}", mitata::fmt_duration(stats.p75)));
  let p99 =
    colors::gray(format!("{:>SUB_W$}", mitata::fmt_duration(stats.p99)));
  let blank_sub = " ".repeat(SUB_W);

  let line1 = format!(
    "{name:<nw$}{GAP}{avg}{GAP}{p75}{GAP}{}",
    rows[0],
    nw = name_width,
  );
  let line2 = format!(
    "{:<nw$}{GAP}{range}{GAP}{p99}{GAP}{}",
    "",
    rows[1],
    nw = name_width,
  );
  let line3 = format!(
    "{:<nw$}{GAP}{iters}{GAP}{blank_sub}{GAP}{}",
    "",
    rows[2],
    nw = name_width,
  );

  // Empty histogram buckets render as spaces; trim so we never emit trailing
  // whitespace on any line.
  [line1, line2, line3]
    .iter()
    .map(|l| l.trim_end())
    .collect::<Vec<_>>()
    .join("\n")
}

/// A single failed benchmark result row.
pub fn benchmark_error(name: &str, message: &str, name_width: usize) -> String {
  format!(
    "{name:<nw$} {}: {message}",
    colors::red("error"),
    nw = name_width,
  )
}

/// Build the right-aligned "(min ... max)" cell with min tinted cyan and max
/// magenta. Padding is computed on the plain text so color escapes don't throw
/// off the alignment.
fn padded_range(min: f64, max: f64) -> String {
  let min = mitata::fmt_duration(min);
  let max = mitata::fmt_duration(max);
  // Width in characters: "(" min " <ellipsis> " max ")".
  let plain_len = 1 + min.chars().count() + 3 + max.chars().count() + 1;
  let pad = MAIN_W.saturating_sub(plain_len);
  format!(
    "{}({} {ELLIPSIS} {})",
    " ".repeat(pad),
    colors::cyan(min),
    colors::magenta(max),
  )
}

#[derive(Clone, Copy, PartialEq)]
enum Region {
  Fast,
  Mean,
  Slow,
}

struct Bins {
  bins: [u64; HIST_W],
  peak: u64,
  mean_bucket: usize,
}

/// Bin the samples into `HIST_W` buckets spanning min .. p99, so outliers past
/// the 99th percentile don't dominate the range. Returns `None` when there
/// isn't a usable spread to draw.
fn compute_bins(samples: &[f64], avg: f64) -> Option<Bins> {
  let n = samples.len();
  if n < 2 {
    return None;
  }
  // samples arrive sorted ascending from the JS side.
  let offset = ((0.99 * (n as f64 - 1.0)) as usize).min(n - 1);
  let min = samples[0];
  let max = samples[offset];
  let step = (max - min) / (HIST_W as f64 - 1.0);
  if step <= 0.0 {
    return None;
  }

  let mut bins = [0u64; HIST_W];
  for &v in &samples[..=offset] {
    let idx = (((v - min) / step).round() as isize)
      .clamp(0, HIST_W as isize - 1) as usize;
    bins[idx] += 1;
  }
  let peak = bins.iter().copied().max().unwrap_or(0);
  if peak == 0 {
    return None;
  }
  let mean_bucket = (((avg - min) / step).round() as isize)
    .clamp(0, HIST_W as isize - 1) as usize;

  Some(Bins {
    bins,
    peak,
    mean_bucket,
  })
}

/// Render the distribution as `HIST_H` stacked, colored rows (top to bottom).
fn histogram(stats: &BenchStats) -> Vec<String> {
  let Some(b) = compute_bins(&stats.samples, stats.avg) else {
    return vec![String::new(); HIST_H];
  };

  // Total fill height of each bucket, in eighths of a cell across all rows.
  let full_scale = (HIST_H * BLOCKS.len()) as f64;
  let region_of = |bucket: usize| {
    if bucket < b.mean_bucket {
      Region::Fast
    } else if bucket == b.mean_bucket {
      Region::Mean
    } else {
      Region::Slow
    }
  };

  let mut rows = Vec::with_capacity(HIST_H);
  // Draw from the top row down so the tallest bars read naturally.
  for row in (0..HIST_H).rev() {
    let mut cells: [(char, Region); HIST_W] = [(' ', Region::Mean); HIST_W];
    for (bucket, &count) in b.bins.iter().enumerate() {
      let eighths =
        ((count as f64 / b.peak as f64) * full_scale).round() as i64;
      let portion = (eighths - (row as i64) * 8).clamp(0, 8);
      let ch = if portion == 0 {
        ' '
      } else {
        BLOCKS[(portion - 1) as usize]
      };
      cells[bucket] = (ch, region_of(bucket));
    }
    rows.push(colorize_cells(&cells));
  }
  rows
}

/// Coalesce adjacent cells sharing a tint into a single color span (spaces stay
/// uncolored) so we emit one escape per run rather than one per character.
fn colorize_cells(cells: &[(char, Region)]) -> String {
  let mut out = String::new();
  let mut i = 0;
  while i < cells.len() {
    // A space carries no tint; treat runs of spaces separately.
    if cells[i].0 == ' ' {
      let start = i;
      while i < cells.len() && cells[i].0 == ' ' {
        i += 1;
      }
      out.push_str(&" ".repeat(i - start));
      continue;
    }
    let region = cells[i].1;
    let mut run = String::new();
    while i < cells.len() && cells[i].0 != ' ' && cells[i].1 == region {
      run.push(cells[i].0);
      i += 1;
    }
    match region {
      Region::Fast => out.push_str(&colors::cyan(run).to_string()),
      Region::Mean => out.push_str(&colors::yellow(run).to_string()),
      Region::Slow => out.push_str(&colors::magenta(run).to_string()),
    }
  }
  out
}

/// One benchmark's identity within a group summary.
pub struct SummaryEntry<'a> {
  pub name: &'a str,
  pub baseline: bool,
  pub avg: f64,
}

/// A horizontal barplot of average times, one bar per benchmark, scaled so the
/// slowest fills the axis.
pub fn barplot(entries: &[SummaryEntry]) -> String {
  const BAR_W: usize = 42;
  let name_w = entries
    .iter()
    .map(|e| e.name.chars().count())
    .max()
    .unwrap_or(0);
  let max_avg = entries.iter().map(|e| e.avg).fold(0.0_f64, f64::max);

  let mut lines = Vec::with_capacity(entries.len());
  for e in entries {
    let frac = if max_avg > 0.0 { e.avg / max_avg } else { 0.0 };
    lines.push(format!(
      "  {:>name_w$} {} {} {}",
      e.name,
      colors::gray("\u{2524}"),
      colors::cyan(hbar(frac, BAR_W)),
      colors::gray(mitata::fmt_duration(e.avg)),
    ));
  }
  lines.join("\n")
}

/// A single horizontal bar filled to `frac` of `width` cells, using partial
/// block glyphs for sub-cell resolution.
fn hbar(frac: f64, width: usize) -> String {
  let eighths = (frac * width as f64 * 8.0).round() as usize;
  let full = eighths / 8;
  let rem = eighths % 8;
  let mut s = "\u{2588}".repeat(full);
  if rem > 0 {
    s.push(HBLOCKS[rem - 1]);
  }
  s
}

/// The "summary" block comparing every benchmark in a group against a baseline
/// (the explicitly-marked one, or the fastest when none is marked).
pub fn summary(entries: &[SummaryEntry]) -> String {
  // Order fastest-first.
  let mut order: Vec<usize> = (0..entries.len()).collect();
  order.sort_by(|&a, &b| entries[a].avg.partial_cmp(&entries[b].avg).unwrap());

  let baseline = order
    .iter()
    .copied()
    .find(|&i| entries[i].baseline)
    .unwrap_or(order[0]);
  let base_avg = entries[baseline].avg;

  let mut s = format!(
    "{}\n  {}",
    colors::gray("summary"),
    colors::cyan_bold(entries[baseline].name),
  );

  for &i in &order {
    if i == baseline {
      continue;
    }
    let avg = entries[i].avg;
    let faster = avg >= base_avg;
    let ratio = mitata::precision_f64(
      if faster {
        avg / base_avg
      } else {
        base_avg / avg
      },
      4,
    );
    let ratio = if ratio > 1000.0 {
      format!("{ratio:>9.0}")
    } else {
      format!("{ratio:>9.2}")
    };
    s.push_str(&format!(
      "\n{}x {} than {}",
      if faster {
        colors::green(ratio)
      } else {
        colors::red(ratio)
      },
      if faster { "faster" } else { "slower" },
      colors::cyan_bold(entries[i].name),
    ));
  }

  s
}
