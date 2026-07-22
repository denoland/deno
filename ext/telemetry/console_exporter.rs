// Copyright 2018-2026 the Deno authors. MIT license.

use std::fmt::Write as _;
use std::io::Write as IoWrite;
use std::time::SystemTime;

use deno_terminal::colors;
use opentelemetry::InstrumentationScope;
use opentelemetry::KeyValue;
use opentelemetry::logs::AnyValue;
use opentelemetry::logs::Severity;
use opentelemetry::trace::SpanId;
use opentelemetry::trace::SpanKind;
use opentelemetry::trace::Status as SpanStatus;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::logs::LogBatch;
use opentelemetry_sdk::logs::SdkLogRecord;
use opentelemetry_sdk::metrics::Temporality;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::Histogram;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use opentelemetry_sdk::trace::SpanData;

// ---- Span Exporter ----

#[derive(Debug)]
pub struct ConsoleSpanExporter {
  resource: Option<Resource>,
}

impl ConsoleSpanExporter {
  pub fn new() -> Self {
    Self { resource: None }
  }
}

impl opentelemetry_sdk::trace::SpanExporter for ConsoleSpanExporter {
  async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
    let resource = self.resource.clone();
    let mut out = String::new();
    for span in &batch {
      format_span(&mut out, span, resource.as_ref());
    }
    let _ = std::io::stderr().write_all(out.as_bytes());
    Ok(())
  }

  fn set_resource(&mut self, resource: &Resource) {
    self.resource = Some(resource.clone());
  }
}

fn format_span(
  out: &mut String,
  span: &SpanData,
  _resource: Option<&Resource>,
) {
  let trace_id = span.span_context.trace_id();
  let span_id = span.span_context.span_id();
  let kind = match span.span_kind {
    SpanKind::Client => "Client",
    SpanKind::Server => "Server",
    SpanKind::Producer => "Producer",
    SpanKind::Consumer => "Consumer",
    SpanKind::Internal => "Internal",
  };

  let duration = span
    .end_time
    .duration_since(span.start_time)
    .unwrap_or_default();
  let duration_ms = duration.as_secs_f64() * 1000.0;

  let _ = writeln!(
    out,
    "{} {} {} {} {}",
    colors::cyan_bold("SPAN"),
    colors::bold(&span.name),
    colors::gray(format!("[{trace_id}/{span_id}]")),
    colors::gray(kind),
    colors::yellow(format!("{duration_ms:.3}ms")),
  );

  if span.parent_span_id != SpanId::INVALID {
    let _ = writeln!(
      out,
      "  {}: {}",
      colors::gray("parent"),
      colors::gray(span.parent_span_id),
    );
  }

  match &span.status {
    SpanStatus::Unset => {}
    SpanStatus::Ok => {
      let _ =
        writeln!(out, "  {}: {}", colors::gray("status"), colors::green("Ok"),);
    }
    SpanStatus::Error { description } => {
      let _ = writeln!(
        out,
        "  {}: {}",
        colors::gray("status"),
        colors::red_bold(format!("Error ({description})")),
      );
    }
  }

  let _ = writeln!(
    out,
    "  {}: {}",
    colors::gray("scope"),
    colors::gray(format_scope(&span.instrumentation_scope)),
  );

  for kv in &span.attributes {
    let _ = writeln!(out, "  {}: {}", colors::cyan(&kv.key), kv.value,);
  }

  if !span.events.is_empty() {
    let _ = writeln!(out, "  {}:", colors::gray("events"));
    for event in span.events.iter() {
      let ts = format_system_time(event.timestamp);
      let _ = write!(
        out,
        "    - {} {}",
        colors::yellow(&event.name),
        colors::gray(format!("({ts})")),
      );
      if !event.attributes.is_empty() {
        let _ = write!(out, " {{");
        for (i, kv) in event.attributes.iter().enumerate() {
          if i > 0 {
            let _ = write!(out, ",");
          }
          let _ = write!(out, " {}: {}", colors::cyan(&kv.key), kv.value);
        }
        let _ = write!(out, " }}");
      }
      let _ = writeln!(out);
    }
  }

  if !span.links.is_empty() {
    let _ = writeln!(out, "  {}:", colors::gray("links"));
    for link in span.links.iter() {
      let _ = writeln!(
        out,
        "    - {}",
        colors::gray(format!(
          "{}/{}",
          link.span_context.trace_id(),
          link.span_context.span_id()
        )),
      );
    }
  }
}

// ---- Log Exporter ----

#[derive(Debug)]
pub struct ConsoleLogExporter {
  resource: Option<Resource>,
}

impl ConsoleLogExporter {
  pub fn new() -> Self {
    Self { resource: None }
  }
}

impl opentelemetry_sdk::logs::LogExporter for ConsoleLogExporter {
  async fn export(&self, batch: LogBatch<'_>) -> OTelSdkResult {
    let mut out = String::new();
    for (record, scope) in batch.iter() {
      format_log(&mut out, record, scope);
    }
    let _ = std::io::stderr().write_all(out.as_bytes());
    Ok(())
  }

  fn set_resource(&mut self, resource: &Resource) {
    self.resource = Some(resource.clone());
  }
}

fn severity_to_str(s: Severity) -> &'static str {
  match s {
    Severity::Trace
    | Severity::Trace2
    | Severity::Trace3
    | Severity::Trace4 => "TRACE",
    Severity::Debug
    | Severity::Debug2
    | Severity::Debug3
    | Severity::Debug4 => "DEBUG",
    Severity::Info | Severity::Info2 | Severity::Info3 | Severity::Info4 => {
      "INFO"
    }
    Severity::Warn | Severity::Warn2 | Severity::Warn3 | Severity::Warn4 => {
      "WARN"
    }
    Severity::Error
    | Severity::Error2
    | Severity::Error3
    | Severity::Error4 => "ERROR",
    Severity::Fatal
    | Severity::Fatal2
    | Severity::Fatal3
    | Severity::Fatal4 => "FATAL",
  }
}

fn format_log(
  out: &mut String,
  record: &SdkLogRecord,
  scope: &InstrumentationScope,
) {
  let severity = record
    .severity_text()
    .or_else(|| record.severity_number().map(severity_to_str))
    .unwrap_or("UNKNOWN");

  let colored_severity: String = match severity {
    "ERROR" | "FATAL" => colors::red_bold(severity).to_string(),
    "WARN" => colors::yellow_bold(severity).to_string(),
    "INFO" => colors::green_bold(severity).to_string(),
    "DEBUG" => colors::cyan(severity).to_string(),
    _ => colors::gray(severity).to_string(),
  };

  let ts = record
    .timestamp()
    .map(format_system_time)
    .unwrap_or_default();

  let body = record.body().map(format_any_value).unwrap_or_default();

  let _ = writeln!(
    out,
    "{} [{}] {} {}",
    colors::green_bold("LOG"),
    colored_severity,
    colors::gray(ts),
    body,
  );
  let _ = writeln!(
    out,
    "  {}: {}",
    colors::gray("scope"),
    colors::gray(format_scope(scope)),
  );

  if let Some(tc) = record.trace_context() {
    let _ = writeln!(
      out,
      "  {}: {}",
      colors::gray("trace"),
      colors::gray(format!("{}/{}", tc.trace_id, tc.span_id)),
    );
  }

  for (key, value) in record.attributes_iter() {
    let _ =
      writeln!(out, "  {}: {}", colors::cyan(key), format_any_value(value),);
  }
}

// ---- Metric Exporter ----

#[derive(Debug)]
pub struct ConsoleMetricExporter {
  temporality: Temporality,
}

impl ConsoleMetricExporter {
  pub fn new(temporality: Temporality) -> Self {
    Self { temporality }
  }
}

impl opentelemetry_sdk::metrics::exporter::PushMetricExporter
  for ConsoleMetricExporter
{
  async fn export(&self, metrics: &ResourceMetrics) -> OTelSdkResult {
    let mut out = String::new();
    for scope_metrics in metrics.scope_metrics() {
      for metric in scope_metrics.metrics() {
        format_metric(&mut out, metric, scope_metrics.scope());
      }
    }
    let _ = std::io::stderr().write_all(out.as_bytes());
    Ok(())
  }

  fn force_flush(&self) -> OTelSdkResult {
    Ok(())
  }

  fn shutdown_with_timeout(
    &self,
    _timeout: std::time::Duration,
  ) -> OTelSdkResult {
    Ok(())
  }

  fn temporality(&self) -> Temporality {
    self.temporality
  }
}

fn format_metric(
  out: &mut String,
  metric: &opentelemetry_sdk::metrics::data::Metric,
  scope: &InstrumentationScope,
) {
  let data = metric.data();

  let kind = match data {
    AggregatedMetrics::F64(d) => metric_data_kind(d),
    AggregatedMetrics::U64(d) => metric_data_kind(d),
    AggregatedMetrics::I64(d) => metric_data_kind(d),
  };

  let unit = if metric.unit().is_empty() {
    String::new()
  } else {
    format!(", unit={}", metric.unit())
  };

  let _ = writeln!(
    out,
    "{} {} {}",
    colors::magenta(colors::bold("METRIC")),
    colors::bold(metric.name()),
    colors::gray(format!("({kind}{unit})")),
  );
  let _ = writeln!(
    out,
    "  {}: {}",
    colors::gray("scope"),
    colors::gray(format_scope(scope)),
  );

  if !metric.description().is_empty() {
    let _ = writeln!(
      out,
      "  {}: {}",
      colors::gray("description"),
      colors::gray(metric.description()),
    );
  }

  match data {
    AggregatedMetrics::F64(d) => format_metric_data(out, d),
    AggregatedMetrics::U64(d) => format_metric_data(out, d),
    AggregatedMetrics::I64(d) => format_metric_data(out, d),
  }
}

fn metric_data_kind<T>(data: &MetricData<T>) -> &'static str {
  match data {
    MetricData::Sum(_) => "Sum",
    MetricData::Gauge(_) => "Gauge",
    MetricData::Histogram(_) => "Histogram",
    MetricData::ExponentialHistogram(_) => "ExponentialHistogram",
  }
}

fn format_metric_data<T: std::fmt::Display + Copy>(
  out: &mut String,
  data: &MetricData<T>,
) {
  match data {
    MetricData::Sum(sum) => {
      let _ = writeln!(
        out,
        "  {} | {}",
        colors::gray(format!(
          "temporality: {}",
          format_temporality(sum.temporality())
        )),
        colors::gray(format!("monotonic: {}", sum.is_monotonic())),
      );
      for dp in sum.data_points() {
        let _ = writeln!(
          out,
          "  {} {}={}",
          format_attributes_colored(dp.attributes()),
          colors::gray("value"),
          colors::yellow(dp.value()),
        );
      }
    }
    MetricData::Gauge(gauge) => {
      for dp in gauge.data_points() {
        let _ = writeln!(
          out,
          "  {} {}={}",
          format_attributes_colored(dp.attributes()),
          colors::gray("value"),
          colors::yellow(dp.value()),
        );
      }
    }
    MetricData::Histogram(hist) => {
      format_histogram(out, hist);
    }
    MetricData::ExponentialHistogram(_) => {}
  }
}

fn format_histogram<T: std::fmt::Display + Copy>(
  out: &mut String,
  hist: &Histogram<T>,
) {
  let _ = writeln!(
    out,
    "  {}",
    colors::gray(format!(
      "temporality: {}",
      format_temporality(hist.temporality())
    )),
  );
  for dp in hist.data_points() {
    let _ = writeln!(
      out,
      "  {} {}={} {}={} {}={} {}={}",
      format_attributes_colored(dp.attributes()),
      colors::gray("count"),
      colors::yellow(dp.count()),
      colors::gray("sum"),
      colors::yellow(dp.sum()),
      colors::gray("min"),
      colors::yellow(
        dp.min()
          .map(|v| v.to_string())
          .unwrap_or_else(|| "-".to_string())
      ),
      colors::gray("max"),
      colors::yellow(
        dp.max()
          .map(|v| v.to_string())
          .unwrap_or_else(|| "-".to_string())
      ),
    );
    let bounds = dp.bounds().collect::<Vec<_>>();
    let counts = dp.bucket_counts().collect::<Vec<_>>();
    let _ = writeln!(out, "    {}  {:?}", colors::gray("bounds:"), bounds);
    let _ = writeln!(out, "    {}  {:?}", colors::gray("counts:"), counts);
  }
}

// ---- Helpers ----

fn format_system_time(t: SystemTime) -> String {
  let dur = t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
  let secs = dur.as_secs();
  let nanos = dur.subsec_nanos();

  const SECS_PER_MIN: u64 = 60;
  const SECS_PER_HOUR: u64 = 3600;
  const SECS_PER_DAY: u64 = 86400;

  let days = secs / SECS_PER_DAY;
  let time_secs = secs % SECS_PER_DAY;
  let hours = time_secs / SECS_PER_HOUR;
  let mins = (time_secs % SECS_PER_HOUR) / SECS_PER_MIN;
  let s = time_secs % SECS_PER_MIN;
  let millis = nanos / 1_000_000;

  let (year, month, day) = days_to_date(days);

  format!(
    "{year:04}-{month:02}-{day:02}T{hours:02}:{mins:02}:{s:02}.{millis:03}Z"
  )
}

fn days_to_date(days_since_epoch: u64) -> (u64, u64, u64) {
  // Algorithm from http://howardhinnant.github.io/date_algorithms.html
  let z = days_since_epoch + 719468;
  let era = z / 146097;
  let doe = z - era * 146097;
  let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
  let y = yoe + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  let mp = (5 * doy + 2) / 153;
  let d = doy - (153 * mp + 2) / 5 + 1;
  let m = if mp < 10 { mp + 3 } else { mp - 9 };
  let y = if m <= 2 { y + 1 } else { y };
  (y, m, d)
}

fn format_scope(scope: &InstrumentationScope) -> String {
  let name = scope.name();
  match scope.version() {
    Some(v) => format!("{name}@{v}"),
    None => name.to_string(),
  }
}

fn format_any_value(value: &AnyValue) -> String {
  match value {
    AnyValue::String(s) => format!("\"{s}\""),
    AnyValue::Int(i) => i.to_string(),
    AnyValue::Double(d) => d.to_string(),
    AnyValue::Boolean(b) => b.to_string(),
    AnyValue::Bytes(b) => format!("{b:?}"),
    AnyValue::ListAny(list) => {
      let items: Vec<String> = list.iter().map(format_any_value).collect();
      format!("[{}]", items.join(", "))
    }
    AnyValue::Map(map) => {
      let items: Vec<String> = map
        .iter()
        .map(|(k, v)| format!("{k}: {}", format_any_value(v)))
        .collect();
      format!("{{{}}}", items.join(", "))
    }
    _ => format!("{value:?}"),
  }
}

fn format_attributes_colored<'a>(
  attrs: impl Iterator<Item = &'a KeyValue>,
) -> String {
  let items: Vec<String> = attrs
    .map(|kv| format!("{}={}", colors::magenta(&kv.key), kv.value))
    .collect();
  if items.is_empty() {
    return colors::gray("{}").to_string();
  }
  format!("{{{}}}", items.join(", "))
}

fn format_temporality(t: Temporality) -> &'static str {
  match t {
    Temporality::Cumulative => "cumulative",
    Temporality::Delta => "delta",
    _ => "unknown",
  }
}
