//! Standalone discovery + command client for a running X-Plane instance,
//! entirely over UDP — no dependency on the X-Plane SDK/`xplm-sys` at all.
//! Meant for tooling that lives *outside* a plugin process (e.g. a hot-reload
//! driver app) and needs to tell an already-running X-Plane to reload plugins
//! it already knows about, without X-Plane itself being the one to invoke
//! this code.
//!
//! This does NOT help with a brand-new plugin install: `sim/operation/reload_plugins`
//! only re-loads plugins X-Plane already found in its startup scan of
//! `Resources/plugins`, it never repeats that scan. A plugin folder that
//! didn't exist at boot stays invisible until X-Plane is actually restarted.
//!
//! X-Plane periodically multicasts a "BECN" beacon packet advertising the UDP
//! port it listens for commands on (its "Network" settings port is
//! user-configurable, so this is the only reliable way to find it without
//! asking the user). Once the port is known, an SDK command such as
//! `sim/operation/reload_plugins` can be triggered with a "CMND" packet.

use std::collections::HashSet;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::time::Instant;

/// Multicast group X-Plane's beacon is broadcast on.
pub const BEACON_MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 1, 1);
/// Port the beacon is broadcast on (fixed by X-Plane, unlike the command port).
pub const BEACON_PORT: u16 = 49707;

/// Parsed contents of an X-Plane "BECN" beacon packet, enough to address the
/// instance directly: the sender's address (from the UDP packet itself) plus
/// the command port it advertises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XPlaneBeacon {
    /// Address to send `CMND`/`DREF`/etc. packets to.
    pub addr: SocketAddr,
    pub beacon_major_version: u8,
    pub beacon_minor_version: u8,
    /// 1 = X-Plane, 2 = PlaneMaker.
    pub application_host_id: i32,
    /// X-Plane version number, e.g. `120100` for 12.1.0.
    pub version_number: u32,
    /// 1 = master, 2 = extern visual, 3 = IOS.
    pub role: u32,
    pub computer_name: String,
}

const BECN_HEADER: &[u8] = b"BECN\0";
// header + major + minor + host_id(4) + version(4) + role(4) + port(2)
const BECN_MIN_LEN: usize = BECN_HEADER.len() + 1 + 1 + 4 + 4 + 4 + 2;

fn parse_becn(buf: &[u8], sender_ip: std::net::IpAddr) -> Option<XPlaneBeacon> {
    if buf.len() < BECN_MIN_LEN || &buf[..BECN_HEADER.len()] != BECN_HEADER {
        return None;
    }
    let mut pos = BECN_HEADER.len();
    let read_u8 = |pos: &mut usize| -> u8 {
        let v = buf[*pos];
        *pos += 1;
        v
    };
    let read_u32 = |pos: &mut usize| -> u32 {
        let v = u32::from_le_bytes(buf[*pos..*pos + 4].try_into().unwrap());
        *pos += 4;
        v
    };
    let read_u16 = |pos: &mut usize| -> u16 {
        let v = u16::from_le_bytes(buf[*pos..*pos + 2].try_into().unwrap());
        *pos += 2;
        v
    };

    let beacon_major_version = read_u8(&mut pos);
    let beacon_minor_version = read_u8(&mut pos);
    let application_host_id = read_u32(&mut pos) as i32;
    let version_number = read_u32(&mut pos);
    let role = read_u32(&mut pos);
    let port = read_u16(&mut pos);

    let name_bytes = &buf[pos..];
    let name_end = name_bytes
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(name_bytes.len());
    let computer_name = String::from_utf8_lossy(&name_bytes[..name_end]).into_owned();

    Some(XPlaneBeacon {
        addr: SocketAddr::new(sender_ip, port),
        beacon_major_version,
        beacon_minor_version,
        application_host_id,
        version_number,
        role,
        computer_name,
    })
}

/// This machine's own IPv4 addresses (one per interface), plus loopback.
/// Used to tell "X-Plane running on this box" apart from some other machine's
/// X-Plane that also happens to be answering the same multicast group on the
/// LAN — the beacon and CMND ports are only meaningful relative to whichever
/// host actually sent them.
fn local_ipv4_addrs() -> io::Result<HashSet<Ipv4Addr>> {
    let mut addrs: HashSet<Ipv4Addr> = local_ip_address::list_afinet_netifas()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?
        .into_iter()
        .filter_map(|(_, ip)| match ip {
            IpAddr::V4(v4) => Some(v4),
            IpAddr::V6(_) => None,
        })
        .collect();
    addrs.insert(Ipv4Addr::LOCALHOST);
    Ok(addrs)
}

/// Listens on the X-Plane beacon multicast group until a beacon from *this
/// machine* is seen or `timeout` elapses. Beacons from other X-Plane
/// instances on the LAN are ignored rather than returned — sending
/// `sim/operation/reload_plugins` to the wrong machine would be silently
/// harmless there but never reach the local install this is meant to reload.
pub async fn discover(timeout: Duration) -> io::Result<XPlaneBeacon> {
    let local_addrs = local_ipv4_addrs()?;

    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, BEACON_PORT)).await?;
    socket.join_multicast_v4(BEACON_MULTICAST_ADDR, Ipv4Addr::UNSPECIFIED)?;

    let deadline = Instant::now() + timeout;
    let mut buf = [0u8; 1024];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "no local X-Plane beacon received within the timeout",
            ));
        }
        let (len, from) = tokio::time::timeout(remaining, socket.recv_from(&mut buf))
            .await
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "no local X-Plane beacon received within the timeout",
                )
            })??;
        let is_local = match from.ip() {
            IpAddr::V4(v4) => local_addrs.contains(&v4),
            IpAddr::V6(_) => false,
        };
        if !is_local {
            continue; // some other machine's X-Plane -- keep listening
        }
        if let Some(beacon) = parse_becn(&buf[..len], from.ip()) {
            return Ok(beacon);
        }
        // Not a BECN packet (or too short to be one) -- keep listening.
    }
}

/// Sends an X-Plane SDK command (e.g. `sim/operation/reload_plugins`) to
/// `target` as a `CMND` UDP packet.
pub async fn send_command(target: SocketAddr, command: &str) -> io::Result<()> {
    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)).await?;
    let mut packet = Vec::with_capacity(5 + command.len() + 1);
    packet.extend_from_slice(b"CMND\0");
    packet.extend_from_slice(command.as_bytes());
    packet.push(0);
    socket.send_to(&packet, target).await?;
    Ok(())
}

/// Discovers a running X-Plane instance and sends it
/// `sim/operation/reload_plugins`, forcing it to unload/reload every plugin
/// it already found in its startup scan of `Resources/plugins` — the same
/// effect as Plugin Admin's manual "Reload Plug-ins" button, usable from a
/// tool that isn't itself a plugin (so can't call `XPLMReloadPlugins`
/// directly). This does NOT make X-Plane discover a plugin folder that
/// didn't exist at boot — that still requires a restart.
pub async fn reload_plugins(discover_timeout: Duration) -> io::Result<()> {
    let beacon = discover(discover_timeout).await?;
    send_command(beacon.addr, "sim/operation/reload_plugins").await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_becn_packet() {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"BECN\0");
        buf.push(1); // major
        buf.push(2); // minor
        buf.extend_from_slice(&1i32.to_le_bytes()); // host id
        buf.extend_from_slice(&120100u32.to_le_bytes()); // version
        buf.extend_from_slice(&1u32.to_le_bytes()); // role
        buf.extend_from_slice(&49000u16.to_le_bytes()); // port
        buf.extend_from_slice(b"MY-PC\0");

        let beacon = parse_becn(&buf, Ipv4Addr::new(192, 168, 1, 10).into()).unwrap();
        assert_eq!(beacon.addr.port(), 49000);
        assert_eq!(beacon.application_host_id, 1);
        assert_eq!(beacon.version_number, 120100);
        assert_eq!(beacon.role, 1);
        assert_eq!(beacon.computer_name, "MY-PC");
    }

    #[test]
    fn rejects_short_or_wrong_header() {
        assert!(parse_becn(b"nope", Ipv4Addr::LOCALHOST.into()).is_none());
        assert!(parse_becn(b"BECN\0short", Ipv4Addr::LOCALHOST.into()).is_none());
    }
}
