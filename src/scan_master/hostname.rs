use std::net::IpAddr;

use dns_lookup::lookup_addr;

pub trait HostnameResolver: Send + Sync + 'static {
    fn resolve(&self, ip: &str) -> Option<String>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemHostnameResolver;

impl HostnameResolver for SystemHostnameResolver {
    fn resolve(&self, ip: &str) -> Option<String> {
        let addr = ip.parse::<IpAddr>().ok()?;
        lookup_addr(&addr).ok()
    }
}
