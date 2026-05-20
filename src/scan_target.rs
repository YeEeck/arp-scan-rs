use std::fmt;
use std::net::Ipv4Addr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    Cidr,
    IpAndMask,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanTargetInput {
    pub mode: InputMode,
    pub cidr_text: String,
    pub ip_text: String,
    pub mask_text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanTargetError(&'static str);

impl fmt::Display for ScanTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for ScanTargetError {}

pub fn normalize_scan_target(input: &ScanTargetInput) -> Result<String, ScanTargetError> {
    match input.mode {
        InputMode::Cidr => normalize_cidr(&input.cidr_text),
        InputMode::IpAndMask => normalize_ip_and_mask(&input.ip_text, &input.mask_text),
    }
}

fn normalize_cidr(cidr_text: &str) -> Result<String, ScanTargetError> {
    let (ip_text, prefix_text) = cidr_text
        .trim()
        .split_once('/')
        .ok_or(ScanTargetError("Invalid CIDR"))?;
    let ip: Ipv4Addr = ip_text.parse().map_err(|_| ScanTargetError("Invalid CIDR"))?;
    let prefix: u8 = prefix_text
        .parse()
        .map_err(|_| ScanTargetError("Invalid CIDR"))?;
    if prefix > 32 {
        return Err(ScanTargetError("Invalid CIDR"));
    }

    let mask = prefix_to_mask(prefix);
    let network = u32::from(ip) & mask;
    Ok(format!("{}/{}", Ipv4Addr::from(network), prefix))
}

fn normalize_ip_and_mask(ip_text: &str, mask_text: &str) -> Result<String, ScanTargetError> {
    let ip: Ipv4Addr = ip_text
        .trim()
        .parse()
        .map_err(|_| ScanTargetError("Invalid IP address"))?;
    let mask_ip: Ipv4Addr = mask_text
        .trim()
        .parse()
        .map_err(|_| ScanTargetError("Invalid subnet mask"))?;
    let mask = u32::from(mask_ip);
    let prefix = mask_to_prefix(mask).ok_or(ScanTargetError("Invalid subnet mask"))?;
    let network = u32::from(ip) & mask;

    Ok(format!("{}/{}", Ipv4Addr::from(network), prefix))
}

fn prefix_to_mask(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        (!0u32) << (32 - prefix)
    }
}

fn mask_to_prefix(mask: u32) -> Option<u8> {
    let prefix = mask.leading_ones() as u8;
    if prefix_to_mask(prefix) == mask {
        Some(prefix)
    } else {
        None
    }
}
