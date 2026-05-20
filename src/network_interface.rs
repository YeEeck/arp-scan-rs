use std::io;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkInterface {
    pub id: String,
    pub name: String,
    pub availability: NetworkInterfaceAvailability,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkInterfaceAvailability {
    Available(NetworkInterfaceIpv4),
    Unavailable { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkInterfaceIpv4 {
    pub ip: String,
    pub mask: String,
    pub cidr: String,
}

pub fn format_interface_label(item: &NetworkInterface) -> String {
    match &item.availability {
        NetworkInterfaceAvailability::Available(_) => item.name.clone(),
        NetworkInterfaceAvailability::Unavailable { reason } => {
            format!("{} ({reason})", item.name)
        }
    }
}

pub fn load_network_interfaces() -> io::Result<Vec<NetworkInterface>> {
    #[cfg(target_os = "windows")]
    {
        load_windows_interfaces()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(vec![NetworkInterface {
            id: "unsupported-platform".into(),
            name: "This platform".into(),
            availability: NetworkInterfaceAvailability::Unavailable {
                reason: "Unsupported platform".into(),
            },
        }])
    }
}

#[cfg(target_os = "windows")]
fn load_windows_interfaces() -> io::Result<Vec<NetworkInterface>> {
    use windows::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS};
    use windows::Win32::NetworkManagement::IpHelper::{
        GAA_FLAG_INCLUDE_PREFIX, GetAdaptersAddresses, IF_TYPE_SOFTWARE_LOOPBACK,
        IP_ADAPTER_ADDRESSES_LH,
    };
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Networking::WinSock::AF_INET;

    let mut buffer_size = 15_000u32;
    let mut buffer = vec![0u8; buffer_size as usize];

    loop {
        let result = unsafe {
            GetAdaptersAddresses(
                AF_INET.0 as u32,
                GAA_FLAG_INCLUDE_PREFIX,
                None,
                Some(buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH),
                &mut buffer_size,
            )
        };

        if result == ERROR_BUFFER_OVERFLOW.0 {
            buffer.resize(buffer_size as usize, 0);
            continue;
        }

        if result != ERROR_SUCCESS.0 {
            return Ok(vec![NetworkInterface {
                id: "interface-load-error".into(),
                name: "Interface loading".into(),
                availability: NetworkInterfaceAvailability::Unavailable {
                    reason: "Failed to load interfaces".into(),
                },
            }]);
        }

        break;
    }

    let mut interfaces = Vec::new();
    let mut current = buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;

    while !current.is_null() {
        let adapter = unsafe { &*current };
        let name = wide_ptr_to_string(adapter.FriendlyName.0);
        let id = ansi_ptr_to_string(adapter.AdapterName.0);

        let availability = if adapter.IfType == IF_TYPE_SOFTWARE_LOOPBACK {
            NetworkInterfaceAvailability::Unavailable {
                reason: "Loopback".into(),
            }
        } else if adapter.OperStatus != IfOperStatusUp {
            NetworkInterfaceAvailability::Unavailable {
                reason: "Interface down".into(),
            }
        } else {
            match first_ipv4_address_and_mask(adapter.FirstUnicastAddress) {
                Some(Ok(ipv4)) => NetworkInterfaceAvailability::Available(ipv4),
                Some(Err(reason)) => NetworkInterfaceAvailability::Unavailable { reason },
                None => NetworkInterfaceAvailability::Unavailable {
                    reason: "No IPv4 address".into(),
                },
            }
        };

        interfaces.push(NetworkInterface {
            id: if id.is_empty() { name.clone() } else { id },
            name,
            availability,
        });

        current = adapter.Next;
    }

    if interfaces.is_empty() {
        interfaces.push(NetworkInterface {
            id: "no-interfaces".into(),
            name: "Network interfaces".into(),
            availability: NetworkInterfaceAvailability::Unavailable {
                reason: "Failed to load interfaces".into(),
            },
        });
    }

    Ok(interfaces)
}

#[cfg(target_os = "windows")]
fn first_ipv4_address_and_mask(
    mut current: *mut windows::Win32::NetworkManagement::IpHelper::IP_ADAPTER_UNICAST_ADDRESS_LH,
) -> Option<Result<NetworkInterfaceIpv4, String>> {
    use std::net::Ipv4Addr;

    use crate::scan_target::convert_ip_and_mask_to_cidr;
    use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR, SOCKADDR_IN};

    while !current.is_null() {
        let unicast = unsafe { &*current };
        let socket_address = unicast.Address;

        if socket_address.lpSockaddr.is_null() {
            current = unicast.Next;
            continue;
        }

        let sockaddr = unsafe { &*(socket_address.lpSockaddr as *const SOCKADDR) };
        if sockaddr.sa_family != AF_INET {
            current = unicast.Next;
            continue;
        }

        let sockaddr_in = unsafe { &*(socket_address.lpSockaddr as *const SOCKADDR_IN) };
        let raw_ip = unsafe { sockaddr_in.sin_addr.S_un.S_addr };
        let ip = Ipv4Addr::from(u32::from_be(raw_ip));
        let prefix = unicast.OnLinkPrefixLength;

        if prefix > 32 {
            return Some(Err("Invalid subnet mask".into()));
        }

        let mask = prefix_to_mask(prefix);
        let mask_text = Ipv4Addr::from(mask).to_string();
        let cidr = match convert_ip_and_mask_to_cidr(&ip.to_string(), &mask_text) {
            Ok(cidr) => cidr,
            Err(_) => return Some(Err("Invalid subnet mask".into())),
        };

        return Some(Ok(NetworkInterfaceIpv4 {
            ip: ip.to_string(),
            mask: mask_text,
            cidr,
        }));
    }

    None
}

#[cfg(target_os = "windows")]
fn prefix_to_mask(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        (!0u32) << (32 - prefix)
    }
}

#[cfg(target_os = "windows")]
fn wide_ptr_to_string(ptr: *mut u16) -> String {
    if ptr.is_null() {
        return String::new();
    }

    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }

        String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
    }
}

#[cfg(target_os = "windows")]
fn ansi_ptr_to_string(ptr: *mut u8) -> String {
    if ptr.is_null() {
        return String::new();
    }

    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }

        String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).to_string()
    }
}
