use std::io;

use super::arp_core;

pub trait ArpProbe {
    fn probe(&self, ip: &str) -> io::Result<Option<[u8; 6]>>;
}

#[derive(Default, Clone, Copy)]
pub struct SystemArpProbe;

impl ArpProbe for SystemArpProbe {
    fn probe(&self, ip: &str) -> io::Result<Option<[u8; 6]>> {
        let ip_addr = arp_core::parse_ip(ip)?;
        arp_core::get_mac_address(ip_addr).map(Some)
    }
}
