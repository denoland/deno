// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Error;
use std::io::Write;
use std::io::{self};
use std::path::Path;
use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_lib::version::DENO_VERSION_INFO;

use super::util;
use super::CoverageReport;
use crate::args::CoverageType;
use crate::colors;

#[derive(Default)]
pub struct CoverageStats<'a> {
  pub line_hit: usize,
  pub line_miss: usize,
  pub branch_hit: usize,
  pub branch_miss: usize,
  pub parent: Option<String>,
  pub file_text: Option<String>,
  pub report: Option<&'a CoverageReport>,
}

type CoverageSummary<'a> = HashMap<String, CoverageStats<'a>>;

pub fn create(kind: CoverageType) -> Box<dyn CoverageReporter + Send> {
  match kind {
    CoverageType::Summary => Box::new(SummaryCoverageReporter::new()),
    CoverageType::Lcov => Box::new(LcovCoverageReporter::new()),
    CoverageType::Detailed => Box::new(DetailedCoverageReporter::new()),
    CoverageType::Html => Box::new(HtmlCoverageReporter::new()),
  }
}

pub trait CoverageReporter {
  fn done(
    &self,
    coverage_root: &Path,
    file_reports: &[(CoverageReport, String)],
  );

  /// Collects the coverage summary of each file or directory.
  fn collect_summary<'a>(
    &'a self,
    file_reports: &'a [(CoverageReport, String)],
  ) -> CoverageSummary<'a> {
    let urls = file_reports.iter().map(|rep| &rep.0.url).collect();
    let root = match util::find_root(urls)
      .and_then(|root_path| root_path.to_file_path().ok())
    {
      Some(path) => path,
      None => return HashMap::new(),
    };
    // summary by file or directory
    // tuple of (line hit, line miss, branch hit, branch miss, parent)
    let mut summary = HashMap::new();
    summary.insert("".to_string(), CoverageStats::default()); // root entry
    for (report, file_text) in file_reports {
      let path = report.url.to_file_path().unwrap();
      let relative_path = path.strip_prefix(&root).unwrap();
      let mut file_text = Some(file_text.to_string());

      let mut summary_path = Some(relative_path);
      // From leaf to root, adds up the coverage stats
      while let Some(path) = summary_path {
        let path_str = path.to_str().unwrap().to_string();
        let parent = path
          .parent()
          .and_then(|p| p.to_str())
          .map(|p| p.to_string());
        let stats = summary.entry(path_str).or_insert(CoverageStats {
          parent,
          file_text,
          report: Some(report),
          ..CoverageStats::default()
        });

        stats.line_hit += report
          .found_lines
          .iter()
          .filter(|(_, count)| *count > 0)
          .count();
        stats.line_miss += report
          .found_lines
          .iter()
          .filter(|(_, count)| *count == 0)
          .count();
        stats.branch_hit += report.branches.iter().filter(|b| b.is_hit).count();
        stats.branch_miss +=
          report.branches.iter().filter(|b| !b.is_hit).count();

        file_text = None;
        summary_path = path.parent();
      }
    }
    summary
  }
}

pub struct SummaryCoverageReporter {}

#[allow(clippy::print_stdout)]
impl SummaryCoverageReporter {
  pub fn new() -> SummaryCoverageReporter {
    SummaryCoverageReporter {}
  }

  fn print_coverage_line(
    &self,
    node: &str,
    node_max: usize,
    stats: &CoverageStats,
  ) {
    let CoverageStats {
      line_hit,
      line_miss,
      branch_hit,
      branch_miss,
      ..
    } = stats;
    let (_, line_percent, line_class) =
      util::calc_coverage_display_info(*line_hit, *line_miss);
    let (_, branch_percent, branch_class) =
      util::calc_coverage_display_info(*branch_hit, *branch_miss);

    let file_name = format!(
      "{node:node_max$}",
      node = node.replace('\\', "/"),
      node_max = node_max
    );
    let file_name = if line_class == "high" {
      format!("{}", colors::green(&file_name))
    } else if line_class == "medium" {
      format!("{}", colors::yellow(&file_name))
    } else {
      format!("{}", colors::red(&file_name))
    };

    let branch_percent = if branch_class == "high" {
      format!("{}", colors::green(&format!("{:>8.1}", branch_percent)))
    } else if branch_class == "medium" {
      format!("{}", colors::yellow(&format!("{:>8.1}", branch_percent)))
    } else {
      format!("{}", colors::red(&format!("{:>8.1}", branch_percent)))
    };

    let line_percent = if line_class == "high" {
      format!("{}", colors::green(&format!("{:>6.1}", line_percent)))
    } else if line_class == "medium" {
      format!("{}", colors::yellow(&format!("{:>6.1}", line_percent)))
    } else {
      format!("{}", colors::red(&format!("{:>6.1}", line_percent)))
    };

    println!(
      " {file_name} | {branch_percent} | {line_percent} |",
      file_name = file_name,
      branch_percent = branch_percent,
      line_percent = line_percent,
    );
  }
}

#[allow(clippy::print_stdout)]
impl CoverageReporter for SummaryCoverageReporter {
  fn done(
    &self,
    _coverage_root: &Path,
    file_reports: &[(CoverageReport, String)],
  ) {
    let summary = self.collect_summary(file_reports);
    let root_stats = summary.get("").unwrap();

    let mut entries = summary
      .iter()
      .filter(|(_, stats)| stats.file_text.is_some())
      .collect::<Vec<_>>();
    entries.sort_by_key(|(node, _)| node.to_owned());
    let node_max = entries
      .iter()
      .map(|(node, _)| node.len())
      .max()
      .unwrap()
      .max("All files".len());

    let header =
      format!("{node:node_max$}  | Branch % | Line % |", node = "File");
    let separator = "-".repeat(header.len());
    println!("{}", separator);
    println!("{}", header);
    println!("{}", separator);
    entries.iter().for_each(|(node, stats)| {
      self.print_coverage_line(node, node_max, stats);
    });
    println!("{}", separator);
    self.print_coverage_line("All files", node_max, root_stats);
    println!("{}", separator);
  }
}

pub struct LcovCoverageReporter {}

impl CoverageReporter for LcovCoverageReporter {
  fn done(
    &self,
    _coverage_root: &Path,
    file_reports: &[(CoverageReport, String)],
  ) {
    file_reports.iter().for_each(|(report, file_text)| {
      self.report(report, file_text).unwrap();
    });
    if let Some((report, _)) = file_reports.first() {
      if let Some(ref output) = report.output {
        if let Ok(path) = output.canonicalize() {
          let url = Url::from_file_path(path).unwrap();
          log::info!("Lcov coverage report has been generated at {}", url);
        } else {
          log::error!(
            "Failed to resolve the output path of Lcov report: {}",
            output.display()
          );
        }
      }
    }
  }
}

impl LcovCoverageReporter {
  pub fn new() -> LcovCoverageReporter {
    LcovCoverageReporter {}
  }

  fn report(
    &self,
    coverage_report: &CoverageReport,
    _file_text: &str,
  ) -> Result<(), AnyError> {
    // pipes output to stdout if no file is specified
    let out_mode: Result<Box<dyn Write>, Error> = match coverage_report.output {
      // only append to the file as the file should be created already
      Some(ref path) => File::options()
        .append(true)
        .open(path)
        .map(|f| Box::new(f) as Box<dyn Write>),
      None => Ok(Box::new(io::stdout())),
    };
    let mut out_writer = out_mode?;

    let file_path = coverage_report
      .url
      .to_file_path()
      .ok()
      .and_then(|p| p.to_str().map(|p| p.to_string()))
      .unwrap_or_else(|| coverage_report.url.to_string());
    writeln!(out_writer, "SF:{file_path}")?;

    for function in &coverage_report.named_functions {
      writeln!(
        out_writer,
        "FN:{},{}",
        function.line_index + 1,
        function.name
      )?;
    }

    for function in &coverage_report.named_functions {
      writeln!(
        out_writer,
        "FNDA:{},{}",
        function.execution_count, function.name
      )?;
    }

    let functions_found = coverage_report.named_functions.len();
    writeln!(out_writer, "FNF:{functions_found}")?;
    let functions_hit = coverage_report
      .named_functions
      .iter()
      .filter(|f| f.execution_count > 0)
      .count();
    writeln!(out_writer, "FNH:{functions_hit}")?;

    for branch in &coverage_report.branches {
      let taken = if let Some(taken) = &branch.taken {
        taken.to_string()
      } else {
        "-".to_string()
      };

      writeln!(
        out_writer,
        "BRDA:{},{},{},{}",
        branch.line_index + 1,
        branch.block_number,
        branch.branch_number,
        taken
      )?;
    }

    let branches_found = coverage_report.branches.len();
    writeln!(out_writer, "BRF:{branches_found}")?;
    let branches_hit =
      coverage_report.branches.iter().filter(|b| b.is_hit).count();
    writeln!(out_writer, "BRH:{branches_hit}")?;
    for (index, count) in &coverage_report.found_lines {
      writeln!(out_writer, "DA:{},{}", index + 1, count)?;
    }

    let lines_hit = coverage_report
      .found_lines
      .iter()
      .filter(|(_, count)| *count != 0)
      .count();
    writeln!(out_writer, "LH:{lines_hit}")?;

    let lines_found = coverage_report.found_lines.len();
    writeln!(out_writer, "LF:{lines_found}")?;

    writeln!(out_writer, "end_of_record")?;
    Ok(())
  }
}

struct DetailedCoverageReporter {}

impl CoverageReporter for DetailedCoverageReporter {
  fn done(
    &self,
    _coverage_root: &Path,
    file_reports: &[(CoverageReport, String)],
  ) {
    file_reports.iter().for_each(|(report, file_text)| {
      self.report(report, file_text).unwrap();
    });
  }
}

#[allow(clippy::print_stdout)]
impl DetailedCoverageReporter {
  pub fn new() -> DetailedCoverageReporter {
    DetailedCoverageReporter {}
  }

  fn report(
    &self,
    coverage_report: &CoverageReport,
    file_text: &str,
  ) -> Result<(), AnyError> {
    let lines = file_text.split('\n').collect::<Vec<_>>();
    print!("cover {} ... ", coverage_report.url);

    let hit_lines = coverage_report
      .found_lines
      .iter()
      .filter(|(_, count)| *count > 0)
      .map(|(index, _)| *index);

    let missed_lines = coverage_report
      .found_lines
      .iter()
      .filter(|(_, count)| *count == 0)
      .map(|(index, _)| *index);

    let lines_found = coverage_report.found_lines.len();
    let lines_hit = hit_lines.count();
    let line_ratio = lines_hit as f32 / lines_found as f32;

    let line_coverage =
      format!("{:.3}% ({}/{})", line_ratio * 100.0, lines_hit, lines_found);

    if line_ratio >= 0.9 {
      println!("{}", colors::green(&line_coverage));
    } else if line_ratio >= 0.75 {
      println!("{}", colors::yellow(&line_coverage));
    } else {
      println!("{}", colors::red(&line_coverage));
    }

    let mut last_line = None;
    for line_index in missed_lines {
      const WIDTH: usize = 4;
      const SEPARATOR: &str = "|";

      // Put a horizontal separator between disjoint runs of lines
      if let Some(last_line) = last_line {
        if last_line + 1 != line_index {
          let dash = colors::gray("-".repeat(WIDTH + 1));
          println!("{}{}{}", dash, colors::gray(SEPARATOR), dash);
        }
      }

      println!(
        "{:width$} {} {}",
        line_index + 1,
        colors::gray(SEPARATOR),
        colors::red(&lines[line_index]),
        width = WIDTH
      );

      last_line = Some(line_index);
    }
    Ok(())
  }
}

pub struct HtmlCoverageReporter {}

impl CoverageReporter for HtmlCoverageReporter {
  fn done(
    &self,
    coverage_root: &Path,
    file_reports: &[(CoverageReport, String)],
  ) {
    let summary = self.collect_summary(file_reports);
    let now = chrono::Utc::now().to_rfc2822();

    for (node, stats) in &summary {
      let report_path =
        self.get_report_path(coverage_root, node, stats.file_text.is_none());
      let main_content = if let Some(file_text) = &stats.file_text {
        self.create_html_code_table(file_text, stats.report.unwrap())
      } else {
        self.create_html_summary_table(node, &summary)
      };
      let is_dir = stats.file_text.is_none();
      let html = self.create_html(node, is_dir, stats, &now, &main_content);
      fs::create_dir_all(report_path.parent().unwrap()).unwrap();
      fs::write(report_path, html).unwrap();
    }

    let root_report = Url::from_file_path(
      coverage_root
        .join("html")
        .join("index.html")
        .canonicalize()
        .unwrap(),
    )
    .unwrap();

    log::info!("HTML coverage report has been generated at {}", root_report);
  }
}

impl HtmlCoverageReporter {
  pub fn new() -> HtmlCoverageReporter {
    HtmlCoverageReporter {}
  }

  /// Gets the report path for a single file
  pub fn get_report_path(
    &self,
    coverage_root: &Path,
    node: &str,
    is_dir: bool,
  ) -> PathBuf {
    if is_dir {
      // e.g. /path/to/coverage/html/src/index.html
      coverage_root.join("html").join(node).join("index.html")
    } else {
      // e.g. /path/to/coverage/html/src/main.ts.html
      Path::new(&format!(
        "{}.html",
        coverage_root.join("html").join(node).to_str().unwrap()
      ))
      .to_path_buf()
    }
  }

  /// Creates single page of html report.
  pub fn create_html(
    &self,
    node: &str,
    is_dir: bool,
    stats: &CoverageStats,
    timestamp: &str,
    main_content: &str,
  ) -> String {
    let title = if node.is_empty() {
      "Coverage report for all files".to_string()
    } else {
      let node = if is_dir {
        format!("{}/", node)
      } else {
        node.to_string()
      };
      format!("Coverage report for {node}")
    };
    let title = title.replace(std::path::MAIN_SEPARATOR, "/");
    let breadcrumbs_parts = node
      .split(std::path::MAIN_SEPARATOR)
      .filter(|s| !s.is_empty())
      .collect::<Vec<_>>();
    let head = self.create_html_head(&title);
    let breadcrumb_navigation =
      self.create_breadcrumbs_navigation(&breadcrumbs_parts, is_dir);
    let header = self.create_html_header(&breadcrumb_navigation, stats);
    let footer = self.create_html_footer(timestamp);
    format!(
      "<!doctype html>
      <html>
        {head}
        <body>
          <div class='wrapper'>
            {header}
            <div class='pad1 overflow-auto'>
              {main_content}
            </div>
            <div class='push'></div>
          </div>
          {footer}
        </body>
      </html>"
    )
  }

  /// Creates <head> tag for html report.
  pub fn create_html_head(&self, title: &str) -> String {
    let style_css = include_str!("style.css");
    format!(
      "
      <head>
        <meta charset='utf-8'>
        <title>{title}</title>
        <style>{style_css}</style>
        <meta name='viewport' content='width=device-width, initial-scale=1' />
      </head>"
    )
  }

  /// Creates header part of the contents for html report.
  pub fn create_html_header(
    &self,
    breadcrumb_navigation: &str,
    stats: &CoverageStats,
  ) -> String {
    let CoverageStats {
      line_hit,
      line_miss,
      branch_hit,
      branch_miss,
      ..
    } = stats;
    let (line_total, line_percent, line_class) =
      util::calc_coverage_display_info(*line_hit, *line_miss);
    let (branch_total, branch_percent, _) =
      util::calc_coverage_display_info(*branch_hit, *branch_miss);

    format!(
      "
      <div class='pad1'>
        <h1>{breadcrumb_navigation}</h1>
        <div class='clearfix'>
          <div class='fl pad1y space-right2'>
            <span class='strong'>{branch_percent:.2}%</span>
            <span class='quiet'>Branches</span>
            <span class='fraction'>{branch_hit}/{branch_total}</span>
          </div>
          <div class='fl pad1y space-right2'>
            <span class='strong'>{line_percent:.2}%</span>
            <span class='quiet'>Lines</span>
            <span class='fraction'>{line_hit}/{line_total}</span>
          </div>
        </div>
      </div>
      <div class='status-line {line_class}'></div>"
    )
  }

  /// Creates footer part of the contents for html report.
  pub fn create_html_footer(&self, now: &str) -> String {
    let version = DENO_VERSION_INFO.deno;
    format!(
      "
      <div class='footer quiet pad2 space-top1 center small'>
        Code coverage generated by
        <a href='https://deno.com/' target='_blank'>Deno v{version}</a>
        at {now}
      </div>"
    )
  }

  /// Creates <table> of summary for html report.
  pub fn create_html_summary_table(
    &self,
    node: &String,
    summary: &CoverageSummary,
  ) -> String {
    let mut children = summary
      .iter()
      .filter(|(_, stats)| stats.parent.as_ref() == Some(node))
      .map(|(k, stats)| (stats.file_text.is_some(), k.clone()))
      .collect::<Vec<_>>();
    // Sort directories first, then files
    children.sort();

    let table_rows: Vec<String> = children.iter().map(|(is_file, c)| {
    let CoverageStats { line_hit, line_miss, branch_hit, branch_miss, .. } =
      summary.get(c).unwrap();

    let (line_total, line_percent, line_class) =
      util::calc_coverage_display_info(*line_hit, *line_miss);
    let (branch_total, branch_percent, branch_class) =
      util::calc_coverage_display_info(*branch_hit, *branch_miss);

    let path = Path::new(c.strip_prefix(&format!("{node}{}", std::path::MAIN_SEPARATOR)).unwrap_or(c)).to_str().unwrap();
    let path = path.replace(std::path::MAIN_SEPARATOR, "/");
    let path_label = if *is_file { path.to_string() } else { format!("{}/", path) };
    let path_link = if *is_file { format!("{}.html", path) } else { format!("{}index.html", path_label) };

    format!("
      <tr>
        <td class='file {line_class}'><a href='{path_link}'>{path_label}</a></td>
        <td class='pic {line_class}'>
          <div class='chart'>
            <div class='cover-fill' style='width: {line_percent:.1}%'></div><div class='cover-empty' style='width: calc(100% - {line_percent:.1}%)'></div>
          </div>
        </td>
        <td class='pct {branch_class}'>{branch_percent:.2}%</td>
        <td class='abs {branch_class}'>{branch_hit}/{branch_total}</td>
        <td class='pct {line_class}'>{line_percent:.2}%</td>
        <td class='abs {line_class}'>{line_hit}/{line_total}</td>
      </tr>")}).collect();
    let table_rows = table_rows.join("\n");

    format!(
      "
      <table class='coverage-summary'>
        <thead>
          <tr>
            <th class='file'>File</th>
            <th class='pic'></th>
            <th class='pct'>Branches</th>
            <th class='abs'></th>
            <th class='pct'>Lines</th>
            <th class='abs'></th>
          </tr>
        </thead>
        <tbody>
          {table_rows}
        </tbody>
      </table>"
    )
  }

  /// Creates <table> of single file code coverage.
  pub fn create_html_code_table(
    &self,
    file_text: &str,
    report: &CoverageReport,
  ) -> String {
    let line_num = file_text.lines().count();
    let line_count = (1..line_num + 1)
      .map(|i| format!("<a name='L{i}'></a><a href='#L{i}'>{i}</a>"))
      .collect::<Vec<_>>()
      .join("\n");
    let line_coverage = (0..line_num)
      .map(|i| {
        if let Some((_, count)) =
          report.found_lines.iter().find(|(line, _)| i == *line)
        {
          if *count == 0 {
            "<span class='cline-any cline-no'>&nbsp</span>".to_string()
          } else {
            format!("<span class='cline-any cline-yes' title='This line is covered {count} time{}'>x{count}</span>", if *count > 1 { "s" } else { "" })
          }
        } else {
          "<span class='cline-any cline-neutral'>&nbsp</span>".to_string()
        }
      })
      .collect::<Vec<_>>()
      .join("\n");
    let branch_coverage = (0..line_num)
      .map(|i| {
        let branch_is_missed = report.branches.iter().any(|b| b.line_index == i && !b.is_hit);
        if branch_is_missed {
          "<span class='missing-if-branch' title='branch condition is missed in this line'>I</span>".to_string()
        } else {
          "".to_string()
        }
      })
      .collect::<Vec<_>>()
      .join("\n");

    let file_text = file_text
      .replace('&', "&amp;")
      .replace('<', "&lt;")
      .replace('>', "&gt;");

    // TODO(kt3k): Add syntax highlight to source code
    format!(
      "<table class='coverage'>
        <tr>
          <td class='line-count quiet'><pre>{line_count}</pre></td>
          <td class='line-coverage quiet'><pre>{line_coverage}</pre></td>
          <td class='branch-coverage quiet'><pre>{branch_coverage}</pre></td>
          <td class='text'><pre class='prettyprint'>{file_text}</pre></td>
        </tr>
      </table>"
    )
  }

  pub fn create_breadcrumbs_navigation(
    &self,
    breadcrumbs_parts: &[&str],
    is_dir: bool,
  ) -> String {
    let mut breadcrumbs_html = Vec::new();
    let root_repeats = if is_dir {
      breadcrumbs_parts.len()
    } else {
      breadcrumbs_parts.len() - 1
    };

    let mut root_url = "../".repeat(root_repeats);
    root_url += "index.html";
    breadcrumbs_html.push(format!("<a href='{root_url}'>All files</a>"));

    for (index, breadcrumb) in breadcrumbs_parts.iter().enumerate() {
      let mut full_url = "../".repeat(breadcrumbs_parts.len() - (index + 1));

      if index == breadcrumbs_parts.len() - 1 {
        breadcrumbs_html.push(breadcrumb.to_string());
        continue;
      }

      if is_dir {
        full_url += "index.html";
      } else {
        full_url += breadcrumb;
        if index != breadcrumbs_parts.len() - 1 {
          full_url += "/index.html";
        }
      }

      breadcrumbs_html.push(format!("<a href='{full_url}'>{breadcrumb}</a>"))
    }

    if breadcrumbs_parts.is_empty() {
      return String::from("All files");
    }

    breadcrumbs_html.into_iter().collect::<Vec<_>>().join(" / ")
  }
}
