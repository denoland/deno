# Deno Permission Audit - Grafana Dashboard

Visualizes Deno permission access audit events emitted via OpenTelemetry.

## Setup

Run Deno with OTel permission auditing enabled:

```bash
OTEL_DENO=true DENO_AUDIT_PERMISSIONS=otel deno run main.ts
```

Set `DENO_TRACE_PERMISSIONS=1` to include stack traces.

## Panels

- **Permission Accesses Over Time** - Stacked bar chart of accesses bucketed by
  the configurable **Interval** variable
- **Permission Access Count by Type** - Donut chart of total accesses by
  permission type over the selected time range
- **Top Accessed Resources** - Table of the 10 most accessed resources
- **Recent Permission Accesses** - Log panel of individual events (expand rows
  for stack traces)
