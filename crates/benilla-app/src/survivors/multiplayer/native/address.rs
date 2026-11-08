//! Direct LAN/Hamachi connections use the same port on every client.
use std::net::{Ipv4Addr, SocketAddr};

pub const PORT: u16 = 8091;

pub fn parse(text: &str) -> Result<SocketAddr, String> {
    let ip: Ipv4Addr = text
        .trim()
        .parse()
        .map_err(|_| "Enter the host's Hamachi IPv4 address (for example: 25.12.34.56).".to_owned())?;
    if ip.is_unspecified() || ip.is_multicast() || ip.is_broadcast() {
        return Err("This IPv4 address does not identify a host.".into());
    }
    Ok(SocketAddr::from((ip, PORT)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hamachi_lan_and_loopback_use_the_fixed_port() {
        for ip in ["25.12.34.56", "192.168.1.20", "127.0.0.1"] {
            assert_eq!(
                parse(&format!(" {ip} ")).unwrap().to_string(),
                format!("{ip}:8091")
            );
        }
    }

    #[test]
    fn only_ipv4_host_addresses_are_accepted() {
        for text in [
            "",
            "AZ-1234",
            "localhost",
            "::1",
            "25.1.2.3:8091",
            "25.1.2.999",
            "0.0.0.0",
            "255.255.255.255",
            "224.0.0.1",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }
}
