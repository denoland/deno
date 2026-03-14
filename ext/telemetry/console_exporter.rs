// Copyright 2018-2026 the Deno authors. MIT license.

use std::fmt::Write as _;
use std::io::Write as IoWrite;
use std::time::SystemTime;

use async_trait::async_trait;
use deno_core::futures::future::BoxFuture;
use opentelemetry::InstrumentationScope;
use opentelemetry::KeyValue;
use opentelemetry::logs::AnyValue;
use opentelemetry::logs::Severity;
use opentelemetry::trace::SpanId;
use opentelemetry::trace::SpanKind;
use opentelemetry::trace::Status as SpanStatus;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::export::logs::LogBatch;
use opentelemetry_sdk::export::trace::SpanData;
use opentelemetry_sdk::logs::LogRecord;
use opentelemetry_sdk::metrics::MetricResult;
use opentelemetry_sdk::metrics::Temporality;
use opentelemetry_sdk::metrics::data::Gauge;
use opentelemetry_sdk::metrics::data::Histogram;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use opentelemetry_sdk::metrics::data::Sum;

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

impl opentelemetry_sdk::export::trace::SpanExporter for ConsoleSpanExporter {
  fn export(
    &mut self,
    batch: Vec<SpanData>,
  ) -> BoxFuture<'static, opentelemetry_sdk::export::trace::ExportResult> {
    let resource = self.resource.clone();
    Box::pin(async move {
      let mut out = String::new();
      for span in &batch {
        format_span(&mut out, span, resource.as_ref());
      }
      let _ = std::io::stderr().write_all(out.as_bytes());
      Ok(())
    })
  }

  fn shutdown(&mut self) {}

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
    "SPAN {name} [{trace_id}/{span_id}] {kind} {duration_ms:.3}ms",
    name = span.name,
  );

  if span.parent_span_id != SpanId::INVALID {
    let _ = writeln!(out, "  parent: {}", span.parent_span_id);
  }

  match &span.status {
    SpanStatus::Unset => {}
    SpanStatus::Ok => {
      let _ = writeln!(out, "  status: Ok");
    }
    SpanStatus::Error { description } => {
      let _ = writeln!(out, "  status: Error ({description})");
    }
  }

  let _ = writeln!(
    out,
    "  scope: {}",
    format_scope(&span.instrumentation_scope)
  );

  for kv in &span.attributes {
    let _ = writeln!(out, "  {}: {}", kv.key, kv.value);
  }

  if !span.events.is_empty() {
    let _ = writeln!(out, "  events:");
    for event in span.events.iter() {
      let ts = format_system_time(event.timestamp);
      let _ = write!(out, "    - {} ({ts})", event.name);
      if !event.attributes.is_empty() {
        let _ = write!(out, " {{");
        for (i, kv) in event.attributes.iter().enumerate() {
          if i > 0 {
            let _ = write!(out, ",");
          }
          let _ = write!(out, " {}: {}", kv.key, kv.value);
        }
        let _ = write!(out, " }}");
      }
      let _ = writeln!(out);
    }
  }

  if !span.links.is_empty() {
    let _ = writeln!(out, "  links:");
    for link in span.links.iter() {
      let _ = writeln!(
        out,
        "    - {}/{}",
        link.span_context.trace_id(),
        link.span_context.span_id()
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

#[async_trait]
impl opentelemetry_sdk::export::logs::LogExporter for ConsoleLogExporter {
  async fn export(
    &mut self,
    batch: LogBatch<'_>,
  ) -> opentelemetry_sdk::export::logs::ExportResult {
    let mut out = String::new();
    for (record, scope) in batch.iter() {
      format_log(&mut out, record, scope);
    }
    let _ = std::io::stderr().write_all(out.as_bytes());
    Ok(())
  }

  fn shutdown(&mut self) {}

  fn set_resource(&mut self, resource: &Resource) {
    self.resource = Some(resource.clone());
  }
}

fn format_log(
  out: &mut String,
  record: &LogRecord,
  scope: &InstrumentationScope,
) {
  let severity = record
    .severity_text
    .or_else(|| {
      record.severity_number.map(|s| match s {
        Severity::Trace
        | Severity::Trace2
        | Severity::Trace3
        | Severity::Trace4 => "TRACE",
        Severity::Debug
        | Severity::Debug2
        | Severity::Debug3
        | Severity::Debug4 => "DEBUG",
        Severity::Info
        | Severity::Info2
        | Severity::Info3
        | Severity::Info4 => "INFO",
        Severity::Warn
        | Severity::Warn2
        | Severity::Warn3
        | Severity::Warn4 => "WARN",
        Severity::Error
        | Severity::Error2
        | Severity::Error3
        | Severity::Error4 => "ERROR",
        Severity::Fatal
        | Severity::Fatal2
        | Severity::Fatal3
        | Severity::Fatal4 => "FATAL",
      })
    })
    .unwrap_or("UNKNOWN");

  let ts = record.timestamp.map(format_system_time).unwrap_or_default();

  let body = record
    .body
    .as_ref()
    .map(format_any_value)
    .unwrap_or_default();

  let _ = writeln!(out, "LOG [{severity}] {ts} {body}");
  let _ = writeln!(out, "  scope: {}", format_scope(scope));

  if let Some(tc) = &record.trace_context {
    let _ = writeln!(out, "  trace: {}/{}", tc.trace_id, tc.span_id);
  }

  for (key, value) in record.attributes_iter() {
    let _ = writeln!(out, "  {key}: {}", format_any_value(value));
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

#[async_trait]
impl opentelemetry_sdk::metrics::exporter::PushMetricExporter
  for ConsoleMetricExporter
{
  async fn export(&self, metrics: &mut ResourceMetrics) -> MetricResult<()> {
    let mut out = String::new();
    for scope_metrics in &metrics.scope_metrics {
      for metric in &scope_metrics.metrics {
        format_metric(&mut out, metric, &scope_metrics.scope);
      }
    }
    let _ = std::io::stderr().write_all(out.as_bytes());
    Ok(())
  }

  async fn force_flush(&self) -> MetricResult<()> {
    Ok(())
  }

  fn shutdown(&self) -> MetricResult<()> {
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
  let data = metric.data.as_any();

  let kind = if data.is::<Sum<f64>>()
    || data.is::<Sum<u64>>()
    || data.is::<Sum<i64>>()
  {
    "Sum"
  } else if data.is::<Gauge<f64>>()
    || data.is::<Gauge<u64>>()
    || data.is::<Gauge<i64>>()
  {
    "Gauge"
  } else if data.is::<Histogram<f64>>()
    || data.is::<Histogram<u64>>()
    || data.is::<Histogram<i64>>()
  {
    "Histogram"
  } else {
    "Unknown"
  };

  let unit = if metric.unit.is_empty() {
    String::new()
  } else {
    format!(", unit={}", metric.unit)
  };

  let _ = writeln!(out, "METRIC {name} ({kind}{unit})", name = metric.name);
  let _ = writeln!(out, "  scope: {}", format_scope(scope));

  if !metric.description.is_empty() {
    let _ = writeln!(out, "  description: {}", metric.description);
  }

  // Sum
  if let Some(sum) = data.downcast_ref::<Sum<f64>>() {
    let _ = writeln!(
      out,
      "  temporality: {} | monotonic: {}",
      format_temporality(sum.temporality),
      sum.is_monotonic
    );
    for dp in &sum.data_points {
      let _ = writeln!(
        out,
        "  {} value={}",
        format_attributes(&dp.attributes),
        dp.value
      );
    }
  } else if let Some(sum) = data.downcast_ref::<Sum<u64>>() {
    let _ = writeln!(
      out,
      "  temporality: {} | monotonic: {}",
      format_temporality(sum.temporality),
      sum.is_monotonic
    );
    for dp in &sum.data_points {
      let _ = writeln!(
        out,
        "  {} value={}",
        format_attributes(&dp.attributes),
        dp.value
      );
    }
  } else if let Some(sum) = data.downcast_ref::<Sum<i64>>() {
    let _ = writeln!(
      out,
      "  temporality: {} | monotonic: {}",
      format_temporality(sum.temporality),
      sum.is_monotonic
    );
    for dp in &sum.data_points {
      let _ = writeln!(
        out,
        "  {} value={}",
        format_attributes(&dp.attributes),
        dp.value
      );
    }
  }

  // Gauge
  if let Some(gauge) = data.downcast_ref::<Gauge<f64>>() {
    for dp in &gauge.data_points {
      let _ = writeln!(
        out,
        "  {} value={}",
        format_attributes(&dp.attributes),
        dp.value
      );
    }
  } else if let Some(gauge) = data.downcast_ref::<Gauge<u64>>() {
    for dp in &gauge.data_points {
      let _ = writeln!(
        out,
        "  {} value={}",
        format_attributes(&dp.attributes),
        dp.value
      );
    }
  } else if let Some(gauge) = data.downcast_ref::<Gauge<i64>>() {
    for dp in &gauge.data_points {
      let _ = writeln!(
        out,
        "  {} value={}",
        format_attributes(&dp.attributes),
        dp.value
      );
    }
  }

  // Histogram
  if let Some(hist) = data.downcast_ref::<Histogram<f64>>() {
    let _ = writeln!(
      out,
      "  temporality: {}",
      format_temporality(hist.temporality)
    );
    for dp in &hist.data_points {
      let _ = writeln!(
        out,
        "  {} count={} sum={} min={} max={}",
        format_attributes(&dp.attributes),
        dp.count,
        dp.sum,
        dp.min
          .map(|v| v.to_string())
          .unwrap_or_else(|| "-".to_string()),
        dp.max
          .map(|v| v.to_string())
          .unwrap_or_else(|| "-".to_string()),
      );
      let _ = writeln!(out, "    bounds:  {:?}", dp.bounds);
      let _ = writeln!(out, "    counts:  {:?}", dp.bucket_counts);
    }
  } else if let Some(hist) = data.downcast_ref::<Histogram<u64>>() {
    let _ = writeln!(
      out,
      "  temporality: {}",
      format_temporality(hist.temporality)
    );
    for dp in &hist.data_points {
      let _ = writeln!(
        out,
        "  {} count={} sum={} min={} max={}",
        format_attributes(&dp.attributes),
        dp.count,
        dp.sum,
        dp.min
          .map(|v| v.to_string())
          .unwrap_or_else(|| "-".to_string()),
        dp.max
          .map(|v| v.to_string())
          .unwrap_or_else(|| "-".to_string()),
      );
      let _ = writeln!(out, "    bounds:  {:?}", dp.bounds);
      let _ = writeln!(out, "    counts:  {:?}", dp.bucket_counts);
    }
  }
}

// ---- Helpers ----

fn format_system_time(t: SystemTime) -> String {
  let dur = t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
  let secs = dur.as_secs();
  let nanos = dur.subsec_nanos();

  // Format as ISO 8601 (simplified — no chrono dependency)
  const SECS_PER_MIN: u64 = 60;
  const SECS_PER_HOUR: u64 = 3600;
  const SECS_PER_DAY: u64 = 86400;

  let days = secs / SECS_PER_DAY;
  let time_secs = secs % SECS_PER_DAY;
  let hours = time_secs / SECS_PER_HOUR;
  let mins = (time_secs % SECS_PER_HOUR) / SECS_PER_MIN;
  let s = time_secs % SECS_PER_MIN;
  let millis = nanos / 1_000_000;

  // Compute date from days since epoch (1970-01-01)
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

fn format_attributes(attrs: &[KeyValue]) -> String {
  if attrs.is_empty() {
    return "{}".to_string();
  }
  let items: Vec<String> = attrs
    .iter()
    .map(|kv| format!("{}={}", kv.key, kv.value))
    .collect();
  format!("{{{}}}", items.join(", "))
}

fn format_temporality(t: Temporality) -> &'static str {
  match t {
    Temporality::Cumulative => "cumulative",
    Temporality::Delta => "delta",
    _ => "unknown",
  }
}
