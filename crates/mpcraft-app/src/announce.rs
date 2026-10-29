//! What answers a DNS-SD search on the local network.
//!
//! Two things MPCraft looks for are published this way: a camera that speaks RTSP
//! (`_rtsp._udp.local.`) and a vehicle that speaks MAVLink (`_mavlink._udp.local.`). Both are read
//! the same way — a name, an address, a port and a text record — so the browsing lives here once and
//! each caller turns what it finds into the thing it is about.
//!
//! The search is link-local by nature: the query goes to `224.0.0.251:5353` with a hop of one and no
//! router forwards it, so only what is on the network this machine is attached to can answer. That is
//! also why a test of it is an advertisement on this machine and not a service on the internet: there
//! is no such thing as mDNS over the internet.
//!
//! This module is the half that needs a network. Everything a caller does with an advertisement is a
//! function of the values below, and is tested with values written down before it is called.

use std::collections::HashMap;
use std::time::Duration;

use thiserror::Error;

/// What one advertisement said, kept as values rather than as a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advert {
    /// The instance name of the service.
    pub id: String,
    /// The host name it answered with.
    pub host: String,
    /// The addresses it resolved to.
    pub addresses: Vec<String>,
    /// The port the service is at.
    pub port: u16,
    /// The TXT properties, in the order they were read.
    pub properties: Vec<(String, String)>,
}

impl Advert {
    /// The address to reach it at, IPv4 first.
    ///
    /// A device that answers with both families is reached over IPv4, because that is what the
    /// registries of a domestic network hand out and what the tools this station runs speak.
    #[must_use]
    pub fn address(&self) -> Option<&String> {
        self.addresses
            .iter()
            .find(|address| !address.contains(':'))
            .or_else(|| self.addresses.first())
    }

    /// What one property says, by a name that is compared without case.
    #[must_use]
    pub fn property(&self, key: &str) -> Option<&str> {
        self.properties
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
            .map(|(_, value)| value.as_str())
    }
}

/// Why a search could not be made.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    /// The platform's mDNS stack could not be opened.
    #[error("the search could not be started: {0}")]
    Start(String),
}

impl DiscoveryError {
    /// The stable code the frontend branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        crate::error::CommandError::CODE_INTERNAL
    }
}

/// Searches the local network for one kind of service.
///
/// # Errors
///
/// Returns [`DiscoveryError::Start`] when the platform's mDNS stack cannot be opened. An empty list
/// is not an error: it says that nothing answered, which is what a network without such a device
/// does.
pub async fn browse(service: &str, timeout: Duration) -> Result<Vec<Advert>, DiscoveryError> {
    let daemon =
        mdns_sd::ServiceDaemon::new().map_err(|error| DiscoveryError::Start(error.to_string()))?;

    let receiver = daemon
        .browse(service)
        .map_err(|error| DiscoveryError::Start(error.to_string()))?;

    let mut found: HashMap<String, Advert> = HashMap::new();
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());

        if left.is_zero() {
            break;
        }

        let event = tokio::time::timeout(left, receiver.recv_async()).await;

        let Ok(Ok(event)) = event else {
            break;
        };

        if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
            let advert = advert_of(&info);

            found.insert(advert.id.clone(), advert);
        }
    }

    // The daemon owns a thread of its own; shutting it down is what lets the search end.
    let _ = daemon.shutdown();

    let mut adverts: Vec<Advert> = found.into_values().collect();

    adverts.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(adverts)
}

/// What one resolved service said, as the callers read it.
fn advert_of(info: &mdns_sd::ResolvedService) -> Advert {
    let properties = info
        .get_properties()
        .iter()
        .map(|property| (property.key().to_owned(), property.val_str().to_owned()))
        .collect();

    let mut addresses: Vec<String> = info
        .get_addresses()
        .iter()
        .map(|address| address.to_ip_addr().to_string())
        .collect();

    addresses.sort();

    Advert {
        id: info.get_fullname().to_owned(),
        host: info.get_hostname().to_owned(),
        addresses,
        port: info.get_port(),
        properties,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advert() -> Advert {
        Advert {
            id: "camera._rtsp._udp.local.".to_owned(),
            host: "camera.local.".to_owned(),
            addresses: vec!["fe80::1".to_owned(), "10.0.0.7".to_owned()],
            port: 554,
            properties: vec![("Name".to_owned(), "Front door".to_owned())],
        }
    }

    #[test]
    fn the_ipv4_address_is_the_one_reaching_it() {
        assert_eq!(advert().address(), Some(&"10.0.0.7".to_owned()));
    }

    #[test]
    fn a_device_that_answered_with_one_family_is_reached_there() {
        let mut one = advert();
        one.addresses = vec!["fe80::1".to_owned()];

        assert_eq!(one.address(), Some(&"fe80::1".to_owned()));
    }

    #[test]
    fn a_property_is_read_without_care_for_case() {
        assert_eq!(advert().property("name"), Some("Front door"));
        assert_eq!(advert().property("path"), None);
    }
}
