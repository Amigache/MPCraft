//! The vehicles that announce themselves on the local network.
//!
//! A vehicle running a companion computer — or a simulator started for a bench — can publish
//! `_mavlink._udp.local.` so that a station does not have to be told where it is. Mission Planner
//! resolves that service and offers to connect (`MainV2.cs`, the `ZeroConf.StartUDPMavlink` handler):
//! it asks before opening anything, and then reaches the address as a UDP client.
//!
//! MPCraft does the same, with one difference of shape: upstream connects from inside the handler and
//! keeps a list of the hosts it has already asked about. Here the search only reports what answered,
//! and the connection is the ordinary one — the operator picks a vehicle, which fills the target and
//! opens it through the same path as an address typed by hand. A network call is still nobody's
//! surprise, and the state of the link is the one the rest of the application already shows.
//!
//! The service is link-local, like every DNS-SD service: it is answered on the network this machine
//! is attached to and nothing forwards it.

use std::time::Duration;

use serde::Serialize;

use crate::announce::{browse, Advert, DiscoveryError};

/// The service type Mission Planner resolves for a vehicle that speaks MAVLink over UDP.
pub const SERVICE_TYPE: &str = "_mavlink._udp.local.";

/// How long a search listens when the caller does not say.
pub const DEFAULT_SEARCH: Duration = Duration::from_secs(6);

/// A vehicle that answered a search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredVehicle {
    /// The instance name of the service.
    pub id: String,
    /// The name the vehicle gives itself, which is what the operator reads.
    pub name: String,
    /// The address it answered with.
    pub address: String,
    /// The port its MAVLink is at.
    pub port: u16,
    /// The target that opens it, in the specification this station parses.
    pub target: String,
}

/// Searches the local network for vehicles.
///
/// # Errors
///
/// Returns [`DiscoveryError::Start`] when the platform's mDNS stack cannot be opened. An empty list
/// is not an error: it says that nothing answered, which is what a network without a vehicle does.
pub async fn discover(timeout: Duration) -> Result<Vec<DiscoveredVehicle>, DiscoveryError> {
    Ok(vehicles_from(browse(SERVICE_TYPE, timeout).await?))
}

/// Reads what a vehicle advertised.
///
/// The name is the `name` property when the vehicle publishes one, and the instance part of the
/// service name otherwise — `SITL` out of `SITL._mavlink._udp.local.` — which is what upstream shows
/// as the display name. A service that resolved to no address is not something to connect to, so it
/// is left out rather than offered.
#[must_use]
pub fn vehicles_from(adverts: Vec<Advert>) -> Vec<DiscoveredVehicle> {
    let mut found = Vec::new();

    for advert in adverts {
        let Some(address) = advert.address() else {
            continue;
        };

        let named = advert.property("name").map(str::to_owned);
        let name = named
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| instance_of(&advert.id));

        let address = address.clone();

        found.push(DiscoveredVehicle {
            name,
            target: format!("udp:{address}:{}", advert.port),
            id: advert.id,
            address,
            port: advert.port,
        });
    }

    found.sort_by(|left, right| left.name.cmp(&right.name));
    found.dedup_by(|left, right| left.id == right.id);

    found
}

/// The instance part of a service name: `SITL._mavlink._udp.local.` is `SITL`.
fn instance_of(fullname: &str) -> String {
    let fullname = fullname.trim_end_matches('.');

    if let Some((instance, _)) = fullname.split_once('.') {
        return instance.to_owned();
    }

    fullname.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advert(properties: Vec<(&str, &str)>) -> Advert {
        Advert {
            id: "SITL._mavlink._udp.local.".to_owned(),
            host: "bench.local.".to_owned(),
            addresses: vec!["192.168.1.50".to_owned()],
            port: 14550,
            properties: properties
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect(),
        }
    }

    #[test]
    fn a_vehicle_is_read_as_a_target_the_station_can_open() {
        let found = vehicles_from(vec![advert(vec![("name", "Bench")])]);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Bench");
        assert_eq!(found[0].address, "192.168.1.50");
        assert_eq!(found[0].port, 14550);
        assert_eq!(found[0].target, "udp:192.168.1.50:14550");
    }

    #[test]
    fn a_vehicle_that_names_nothing_is_read_by_its_instance() {
        let found = vehicles_from(vec![advert(vec![])]);

        assert_eq!(found[0].name, "SITL");
    }

    #[test]
    fn the_ipv4_address_is_the_one_it_is_reached_at() {
        let mut service = advert(vec![("name", "Bench")]);
        service.addresses = vec!["fe80::2".to_owned(), "10.0.0.9".to_owned()];

        let found = vehicles_from(vec![service]);

        assert_eq!(found[0].target, "udp:10.0.0.9:14550");
    }

    #[test]
    fn a_service_that_resolved_to_no_address_is_not_offered() {
        let mut service = advert(vec![("name", "Bench")]);
        service.addresses.clear();

        assert!(vehicles_from(vec![service]).is_empty());
    }
}
