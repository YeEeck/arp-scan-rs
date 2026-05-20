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
    Ok(vec![NetworkInterface {
        id: "loading-not-implemented".into(),
        name: "Interface loading".into(),
        availability: NetworkInterfaceAvailability::Unavailable {
            reason: "Failed to load interfaces".into(),
        },
    }])
}
