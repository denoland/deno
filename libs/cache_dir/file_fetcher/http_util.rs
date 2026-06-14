// Copyright 2018-2026 the Deno authors. MIT license.

use std::time::Duration;
use std::time::SystemTime;

use chrono::DateTime;

use crate::common::HeadersMap;

#[derive(Eq, PartialEq)]
enum Cachability {
  Public,
  Private,
  NoCache,
  OnlyIfCached,
}

#[derive(Default)]
struct CacheControl {
  cachability: Option<Cachability>,
  max_age: Option<Duration>,
  max_stale: Option<Duration>,
  min_fresh: Option<Duration>,
}

impl CacheControl {
  fn from_value(value: &str) -> Option<Self> {
    let mut ret = Self::default();
    for token in value.split(',') {
      let (key, val) = {
        let mut split = token.split('=').map(|s| s.trim());
        (split.next().unwrap(), split.next())
      };

      match key {
        "public" => ret.cachability = Some(Cachability::Public),
        "private" => ret.cachability = Some(Cachability::Private),
        "no-cache" => ret.cachability = Some(Cachability::NoCache),
        "only-if-cached" => ret.cachability = Some(Cachability::OnlyIfCached),
        "max-age" => match val.and_then(|v| v.parse().ok()) {
          Some(secs) => ret.max_age = Some(Duration::from_secs(secs)),
          None => return None,
        },
        "max-stale" => match val.and_then(|v| v.parse().ok()) {
          Some(secs) => ret.max_stale = Some(Duration::from_secs(secs)),
          None => return None,
        },
        "min-fresh" => match val.and_then(|v| v.parse().ok()) {
          Some(secs) => ret.min_fresh = Some(Duration::from_secs(secs)),
          None => return None,
        },
        _ => (),
      };
    }
    Some(ret)
  }
}

/// A structure used to determine if a entity in the http cache can be used.
///
/// This is heavily influenced by
/// <https://github.com/kornelski/rusty-http-cache-semantics> which is BSD
/// 2-Clause Licensed and copyright Kornel Lesiński
pub struct CacheSemantics {
  cache_control: CacheControl,
  cached: SystemTime,
  headers: HeadersMap,
  now: SystemTime,
}

impl CacheSemantics {
  pub fn new(headers: HeadersMap, cached: SystemTime, now: SystemTime) -> Self {
    let cache_control = headers
      .get("cache-control")
      .map(|v| CacheControl::from_value(v).unwrap_or_default())
      .unwrap_or_default();
    Self {
      cache_control,
      cached,
      headers,
      now,
    }
  }

  fn age(&self) -> Duration {
    let mut age = self.age_header_value();

    if let Ok(resident_time) = self.now.duration_since(self.cached) {
      age += resident_time;
    }

    age
  }

  fn age_header_value(&self) -> Duration {
    Duration::from_secs(
      self
        .headers
        .get("age")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0),
    )
  }

  fn is_stale(&self) -> bool {
    self.max_age() <= self.age()
  }

  fn max_age(&self) -> Duration {
    if self.cache_control.cachability == Some(Cachability::NoCache) {
      return Duration::from_secs(0);
    }

    if self.headers.get("vary").map(|s| s.trim()) == Some("*") {
      return Duration::from_secs(0);
    }

    if let Some(max_age) = self.cache_control.max_age {
      return max_age;
    }

    let default_min_ttl = Duration::from_secs(0);

    let server_date = self.raw_server_date();
    if let Some(expires) = self.headers.get("expires") {
      return match DateTime::parse_from_rfc2822(expires) {
        Err(_) => Duration::from_secs(0),
        Ok(expires) => {
          let expires = SystemTime::UNIX_EPOCH
            + Duration::from_secs(expires.timestamp().max(0) as _);
          return default_min_ttl
            .max(expires.duration_since(server_date).unwrap_or_default());
        }
      };
    }

    if let Some(last_modified) = self.headers.get("last-modified")
      && let Ok(last_modified) = DateTime::parse_from_rfc2822(last_modified)
    {
      let last_modified = SystemTime::UNIX_EPOCH
        + Duration::from_secs(last_modified.timestamp().max(0) as _);
      if let Ok(diff) = server_date.duration_since(last_modified) {
        let secs_left = diff.as_secs() as f64 * 0.1;
        return default_min_ttl.max(Duration::from_secs(secs_left as _));
      }
    }

    default_min_ttl
  }

  fn raw_server_date(&self) -> SystemTime {
    self
      .headers
      .get("date")
      .and_then(|d| DateTime::parse_from_rfc2822(d).ok())
      .and_then(|d| {
        SystemTime::UNIX_EPOCH
          .checked_add(Duration::from_secs(d.timestamp() as _))
      })
      .unwrap_or(self.cached)
  }

  /// Returns true if the cached value is "fresh" respecting cached headers,
  /// otherwise returns false.
  pub fn should_use(&self) -> bool {
    if self.cache_control.cachability == Some(Cachability::NoCache) {
      return false;
    }

    if let Some(max_age) = self.cache_control.max_age
      && self.age() > max_age
    {
      return false;
    }

    if let Some(min_fresh) = self.cache_control.min_fresh
      && self.time_to_live() < min_fresh
    {
      return false;
    }

    if self.is_stale() {
      let has_max_stale = self.cache_control.max_stale.is_some();
      let allows_stale = has_max_stale
        && self
          .cache_control
          .max_stale
          .map(|val| val > self.age() - self.max_age())
          .unwrap_or(true);
      if !allows_stale {
        return false;
      }
    }

    true
  }

  fn time_to_live(&self) -> Duration {
    self.max_age().checked_sub(self.age()).unwrap_or_default()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn cache_control_parse_freshness_directives() {
    let cache_control =
      CacheControl::from_value("public, max-age=60, max-stale=10, min-fresh=5")
        .unwrap();
    assert!(matches!(
      cache_control.cachability,
      Some(Cachability::Public)
    ));
    assert_eq!(cache_control.max_age, Some(Duration::from_secs(60)));
    assert_eq!(cache_control.max_stale, Some(Duration::from_secs(10)));
    assert_eq!(cache_control.min_fresh, Some(Duration::from_secs(5)));
  }

  #[test]
  fn cache_control_parse_matches_dependency_quirks() {
    assert!(CacheControl::from_value("max-age=bad, no-cache").is_none());
    assert!(CacheControl::from_value("max-stale").is_none());

    let cache_control =
      CacheControl::from_value("no-cache, public, max-age=60=ignored").unwrap();
    assert!(matches!(
      cache_control.cachability,
      Some(Cachability::Public)
    ));
    assert_eq!(cache_control.max_age, Some(Duration::from_secs(60)));
  }
}
