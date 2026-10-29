//! The cameras that announce themselves on the local network.
//!
//! A camera that speaks RTSP often publishes itself with DNS-SD (mDNS, "Bonjour", "Zeroconf") so
//! that a client does not have to be told its address. Mission Planner resolves the service type
//! `_rtsp._udp.local.` (`ExtLibs/Utilities/ZeroConf.cs`) and builds one button per resolution the
//! camera advertises in its TXT record.
//!
//! Discovery is link-local by nature: the query goes to `224.0.0.251:5353` with a TTL of one hop and
//! no router forwards it, so a camera can only be found on the network this machine is attached to.
//! That is why the tests below are a scripted advertisement and an advertisement from another
//! implementation on this machine, rather than a service on the internet: there is no such thing.
//!
//! The reading is kept in two halves. The parsing is a function of what was advertised — a name, an
//! address, a port, a path and the properties — and is tested with values written down before it is
//! called. The browsing is the part that needs a network, and it is the thin layer that turns an
//! `mdns-sd` answer into those values.

use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;
use thiserror::Error;

/// The service type Mission Planner resolves for a camera that publishes RTSP.
pub const SERVICE_TYPE: &str = "_rtsp._udp.local.";

/// How long a search listens when the caller does not say.
pub const DEFAULT_SEARCH: Duration = Duration::from_secs(6);

/// One way of asking a camera for a picture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraStream {
    /// The codec the camera named, when it named one.
    pub codec: String,
    /// The width it advertised, when it advertised a size.
    pub width: Option<u16>,
    /// The height it advertised, when it advertised a size.
    pub height: Option<u16>,
    /// The address the relay reads.
    pub url: String,
}

/// A camera that answered a search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraHost {
    /// The instance name of the service.
    pub id: String,
    /// The name the camera gives itself, which is what the operator reads.
    pub name: String,
    /// The host name it answers with.
    pub host: String,
    /// The address the stream is at.
    pub address: String,
    /// The port the stream is at.
    pub port: u16,
    /// The path of the stream on that port, when it advertised one.
    pub path: String,
    /// The streams it advertised.
    pub streams: Vec<CameraStream>,
}

/// What one advertisement said, kept as values rather than as a record.
///
/// This is the seam the parsing is tested at: an mDNS answer is turned into this, and everything
/// after it is a function of these fields.
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

/// Why a search could not be made.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    /// The platform's mDNS stack could not be opened.
    #[error("the search for cameras could not be started: {0}")]
    Start(String),
}

impl DiscoveryError {
    /// The stable code the frontend branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        crate::error::CommandError::CODE_INTERNAL
    }
}

/// Reads what a camera advertised.
///
/// The properties are the vocabulary upstream reads as well: `name` is what the operator sees,
/// `path` is the part of the address after the port, and any value that carries a codec followed by
/// its resolutions — `H264(1920x1080,1280x720)` — is one entry per size. A camera that advertises no
/// size still gets one entry: the stream is there, and refusing it because the text did not say how
/// big the picture is would be refusing a camera that works.
#[must_use]
pub fn hosts_from(adverts: Vec<Advert>) -> Vec<CameraHost> {
    let mut hosts = Vec::new();

    for advert in adverts {
        // The first address is the one the stream is read at. A camera that answers with both
        // families is read over IPv4 first, because the relay's `ffmpeg` and a camera on a domestic
        // network both speak that; the rest are kept by the network, not by the interface.
        let Some(address) = advert
            .addresses
            .iter()
            .find(|address| !address.contains(':'))
            .or_else(|| advert.addresses.first())
        else {
            continue;
        };

        let mut name = String::new();
        let mut path = String::new();

        // The name and the path are read before the streams, because the address of a stream is
        // written from the path and a camera is not obliged to list its properties in an order.
        for (key, value) in &advert.properties {
            match key.to_ascii_lowercase().as_str() {
                "name" => name = value.clone(),
                "path" => path = normalise_path(value),
                _ => {}
            }
        }

        let mut streams = Vec::new();

        for (key, value) in &advert.properties {
            match key.to_ascii_lowercase().as_str() {
                "name" | "path" => {}
                _ => streams.extend(streams_of(value, address, advert.port, &path)),
            }
        }

        if streams.is_empty() {
            streams.push(stream(String::new(), None, address, advert.port, &path));
        }

        if name.trim().is_empty() {
            name = advert.id.clone();
        }

        hosts.push(CameraHost {
            id: advert.id,
            name,
            host: advert.host,
            address: address.clone(),
            port: advert.port,
            path,
            streams,
        });
    }

    hosts.sort_by(|left, right| left.name.cmp(&right.name));
    hosts.dedup_by(|left, right| left.id == right.id);

    hosts
}

/// A path as the address needs it: leading slash, no trailing spaces.
fn normalise_path(path: &str) -> String {
    let path = path.trim();

    if path.is_empty() {
        String::new()
    } else if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    }
}

/// The streams a property value advertises.
///
/// A value is read as a codec followed by a parenthesised list: `H264(1920x1080,1280x720)`. A value
/// that carries no size is not a stream, and is left alone rather than guessed at.
fn streams_of(value: &str, address: &str, port: u16, path: &str) -> Vec<CameraStream> {
    let mut streams = Vec::new();
    let text = value.trim();
    let Some(open) = text.find('(') else {
        return streams;
    };
    let Some(close) = text[open..].find(')').map(|at| open + at) else {
        return streams;
    };

    let codec = text[..open].trim().to_owned();
    let inside = &text[open + 1..close];

    for size in inside.split(',') {
        let size = size.trim();
        let Some((width, height)) = size.split_once('x') else {
            continue;
        };

        let (Ok(width), Ok(height)) = (width.trim().parse::<u16>(), height.trim().parse::<u16>())
        else {
            continue;
        };

        if width == 0 || height == 0 {
            continue;
        }

        streams.push(stream(
            codec.clone(),
            Some((width, height)),
            address,
            port,
            path,
        ));
    }

    streams
}

/// One entry of the list, with the address the relay reads.
fn stream(
    codec: String,
    size: Option<(u16, u16)>,
    address: &str,
    port: u16,
    path: &str,
) -> CameraStream {
    let mut url = format!("rtsp://{address}:{port}{path}");

    // The query is how an Axis camera is asked for a size, which is the shape upstream builds.
    if let Some((width, height)) = size {
        url.push_str(&format!("?width={width}&height={height}"));
    }

    CameraStream {
        codec,
        width: size.map(|(width, _)| width),
        height: size.map(|(_, height)| height),
        url,
    }
}

/// Searches the local network for cameras.
///
/// # Errors
///
/// Returns [`DiscoveryError::Start`] when the platform's mDNS stack cannot be opened. An empty list
/// is not an error: it says that nothing answered, which is what a network without a camera does.
pub async fn discover(timeout: Duration) -> Result<Vec<CameraHost>, DiscoveryError> {
    let daemon =
        mdns_sd::ServiceDaemon::new().map_err(|error| DiscoveryError::Start(error.to_string()))?;

    let receiver = daemon
        .browse(SERVICE_TYPE)
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

    Ok(hosts_from(found.into_values().collect()))
}

/// What one resolved service said, as the parsing reads it.
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

    /// What a camera of the kind upstream draws advertises.
    fn advert(properties: Vec<(&str, &str)>) -> Advert {
        Advert {
            id: "AXIS M3045._rtsp._udp.local.".to_owned(),
            host: "axis-m3045.local.".to_owned(),
            addresses: vec!["192.168.1.40".to_owned()],
            port: 554,
            properties: properties
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect(),
        }
    }

    #[test]
    fn a_camera_that_advertises_two_sizes_is_read_as_two_streams() {
        let hosts = hosts_from(vec![advert(vec![
            ("name", "Front door"),
            ("path", "axis-media/media.amp"),
            ("codecs", "H264(1920x1080,1280x720)"),
        ])]);

        assert_eq!(hosts.len(), 1);

        let host = &hosts[0];

        assert_eq!(host.name, "Front door");
        assert_eq!(host.address, "192.168.1.40");
        assert_eq!(host.port, 554);
        assert_eq!(host.path, "/axis-media/media.amp");
        assert_eq!(host.streams.len(), 2);
        assert_eq!(host.streams[0].codec, "H264");
        assert_eq!(host.streams[0].width, Some(1920));
        assert_eq!(host.streams[0].height, Some(1080));
        assert_eq!(
            host.streams[0].url,
            "rtsp://192.168.1.40:554/axis-media/media.amp?width=1920&height=1080"
        );
        assert_eq!(
            host.streams[1].url,
            "rtsp://192.168.1.40:554/axis-media/media.amp?width=1280&height=720"
        );
    }

    #[test]
    fn a_camera_that_advertises_no_size_is_still_a_camera() {
        let hosts = hosts_from(vec![advert(vec![("name", "Roof")])]);

        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].streams.len(), 1);
        assert_eq!(hosts[0].streams[0].width, None);
        assert_eq!(hosts[0].streams[0].url, "rtsp://192.168.1.40:554");
        assert_eq!(hosts[0].name, "Roof");
    }

    #[test]
    fn a_name_is_the_instance_when_the_camera_names_nothing() {
        let hosts = hosts_from(vec![advert(vec![])]);

        assert_eq!(hosts[0].name, "AXIS M3045._rtsp._udp.local.");
    }

    #[test]
    fn a_value_that_says_nothing_about_a_size_is_not_read_as_one() {
        let hosts = hosts_from(vec![advert(vec![
            ("name", "Door"),
            ("location", "Lobby"),
            ("codecs", "H264(nonsense)"),
        ])]);

        // The camera still has a stream: what it did not say is how big the picture is.
        assert_eq!(hosts[0].streams.len(), 1);
        assert_eq!(hosts[0].streams[0].width, None);
    }

    #[test]
    fn the_ipv4_address_is_the_one_the_stream_is_read_at() {
        let mut service = advert(vec![("name", "Door")]);
        service.addresses = vec!["fe80::1".to_owned(), "10.0.0.7".to_owned()];

        let hosts = hosts_from(vec![service]);

        assert_eq!(hosts[0].address, "10.0.0.7");
        assert_eq!(hosts[0].streams[0].url, "rtsp://10.0.0.7:554");
    }

    #[test]
    fn a_service_that_resolved_to_no_address_is_not_a_camera() {
        let mut service = advert(vec![("name", "Door")]);
        service.addresses.clear();

        assert!(hosts_from(vec![service]).is_empty());
    }
}
