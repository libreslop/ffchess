//! Origin check of websocket upgrades.
//!
//! Browsers attach the page's origin to websocket upgrades but do not apply CORS to them, so
//! without this check any website could open game connections from its visitors' browsers.

use axum::http::HeaderMap;
use axum::http::header::{HOST, ORIGIN};

/// Header naming the host a client asked a reverse proxy for.
const FORWARDED_HOST: &str = "x-forwarded-host";

/// Where an upgrade request comes from and which host it was sent to.
pub(super) struct UpgradeOrigin<'a> {
    /// The page origin, absent for clients that are not browsers.
    origin: Option<&'a str>,
    /// Hosts the request was addressed to, directly or through a proxy.
    hosts: Vec<&'a str>,
}

impl<'a> UpgradeOrigin<'a> {
    /// Reads the origin and hosts of an upgrade request.
    pub(super) fn of(headers: &'a HeaderMap) -> Self {
        let header = |name| headers.get(name).and_then(|value| value.to_str().ok());
        let hosts = [header(HOST.as_str()), header(FORWARDED_HOST)]
            .into_iter()
            .flatten()
            .flat_map(|value| value.split(','))
            .map(str::trim)
            .collect();
        Self {
            origin: header(ORIGIN.as_str()),
            hosts,
        }
    }

    /// Returns true for pages served by this server or listed in `allowed`.
    ///
    /// Requests without an origin come from clients that are not browsers. Those can claim any
    /// origin anyway, so the check only matters for browsers and lets them through.
    pub(super) fn is_allowed(&self, allowed: &[String]) -> bool {
        let Some(origin) = self.origin else {
            return true;
        };
        let listed = allowed
            .iter()
            .any(|entry| entry.trim_end_matches('/').eq_ignore_ascii_case(origin));
        let same_host = origin.split_once("://").is_some_and(|(_, authority)| {
            self.hosts
                .iter()
                .any(|host| host.eq_ignore_ascii_case(authority))
        });
        listed || same_host
    }
}
