use pnet_packet::icmp::IcmpPacket;
use pnet_packet::icmp::IcmpTypes;
use pnet_packet::icmpv6::Icmpv6Packet;
use pnet_packet::icmpv6::Icmpv6Types;
use pnet_packet::ip::IpNextHeaderProtocols;
use pnet_packet::ipv4::Ipv4Packet;
use pnet_packet::ipv6::Ipv6Packet;
use pnet_packet::tcp::TcpPacket;
use pnet_packet::udp::UdpPacket;
use pnet_packet::Packet as IpPacket;
use ratatui::prelude::Alignment;
use ratatui::prelude::Stylize;
use ratatui::style::Style;
use ratatui::text::Text;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

pub enum PacketDirection {
    ToNode6(Ipv6Packet<'static>),
    ToHost6(Ipv6Packet<'static>),
    ToNode4(Ipv4Packet<'static>),
    ToHost4(Ipv4Packet<'static>),
}

impl PacketDirection {
    pub fn get_title(&self) -> String {
        match self {
            Self::ToNode6(packet) => {
                format!(
                    "Host [{:}] -> Node [{:}] {:} bytes",
                    packet.get_source(),
                    packet.get_destination(),
                    packet.packet().len()
                )
            }
            Self::ToHost6(packet) => {
                format!(
                    "Host [{:}] <- Node [{:}] {:} bytes",
                    packet.get_destination(),
                    packet.get_source(),
                    packet.packet().len(),
                )
            }
            Self::ToNode4(packet) => {
                format!(
                    "Host [{:}] -> Node [{:}] {:} bytes",
                    packet.get_source(),
                    packet.get_destination(),
                    packet.packet().len()
                )
            }
            Self::ToHost4(packet) => {
                format!(
                    "Host [{:}] <- Node [{:}] {:} bytes",
                    packet.get_destination(),
                    packet.get_source(),
                    packet.packet().len(),
                )
            }
        }
    }

    pub fn get_payload(&self) -> String {
        match self {
            Self::ToNode6(packet) | Self::ToHost6(packet) => {
                let protocol = packet.get_next_header();
                let protocol_info = match protocol {
                    IpNextHeaderProtocols::Icmpv6 => {
                        match Icmpv6Packet::new(packet.payload())
                            .unwrap()
                            .get_icmpv6_type()
                        {
                            Icmpv6Types::EchoRequest => "EchoRequest",
                            Icmpv6Types::EchoReply => "EchoReply",
                            _ => "",
                        }
                    }
                    IpNextHeaderProtocols::Tcp => {
                        let tcp = TcpPacket::new(packet.payload()).unwrap();
                        &format!("port {:}", tcp.get_destination())
                    }
                    IpNextHeaderProtocols::Udp => {
                        let ucp = UdpPacket::new(packet.payload()).unwrap();
                        &format!("port {:}", ucp.get_destination())
                    }
                    _ => "",
                };

                format!(
                    "{:} {:} TTL {:3}; {:} bytes",
                    protocol,
                    protocol_info,
                    packet.get_hop_limit(),
                    packet.get_payload_length()
                )
            }
            Self::ToNode4(packet) | Self::ToHost4(packet) => {
                let protocol = packet.get_next_level_protocol();
                let protocol_info = match protocol {
                    IpNextHeaderProtocols::Icmp => {
                        match IcmpPacket::new(packet.payload()).unwrap().get_icmp_type() {
                            IcmpTypes::EchoRequest => "EchoRequest",
                            IcmpTypes::EchoReply => "EchoReply",
                            _ => "",
                        }
                    }
                    IpNextHeaderProtocols::Tcp => {
                        let tcp = TcpPacket::new(packet.payload()).unwrap();
                        &format!("port {:}", tcp.get_destination())
                    }
                    IpNextHeaderProtocols::Udp => {
                        let ucp = UdpPacket::new(packet.payload()).unwrap();
                        &format!("port {:}", ucp.get_destination())
                    }
                    _ => "",
                };

                format!(
                    "{:} {:} TTL {:3}; {:} bytes",
                    protocol,
                    protocol_info,
                    packet.get_ttl(),
                    packet.get_total_length()
                )
            }
        }
    }

    pub fn paragraph(&self) -> (usize, Paragraph<'_>) {
        let block = Block::new()
            .borders(Borders::TOP | Borders::BOTTOM)
            .style(Style::new().gray())
            .title(self.get_title())
            .title_alignment(Alignment::Left);

        let text = Text::from(self.get_payload()).reset_style();

        let size = text.lines.len() + 2;
        (size, Paragraph::new(text).block(block))
    }
}

pub struct PacketLog {
    log: Vec<PacketDirection>,
}

impl PacketLog {
    pub fn packet_to_host(packet: &[u8]) -> Result<PacketDirection, ()> {
        match packet[0] >> 4 {
            // IPv4
            4 => Ok(PacketDirection::ToHost4(
                Ipv4Packet::owned(packet.to_vec()).unwrap(),
            )),
            // IPv6
            6 => Ok(PacketDirection::ToHost6(
                Ipv6Packet::owned(packet.to_vec()).unwrap(),
            )),
            _ => Err(()),
        }
    }
    pub fn packet_to_node(packet: &[u8]) -> Result<PacketDirection, ()> {
        match packet[0] >> 4 {
            // IPv4
            4 => Ok(PacketDirection::ToNode4(
                Ipv4Packet::owned(packet.to_vec()).unwrap(),
            )),
            // IPv6
            6 => Ok(PacketDirection::ToNode6(
                Ipv6Packet::owned(packet.to_vec()).unwrap(),
            )),
            _ => Err(()),
        }
    }

    pub const fn new() -> Self {
        Self { log: vec![] }
    }
    pub fn add_to_host(&mut self, packet: &[u8]) {
        if let Ok(packet) = Self::packet_to_host(packet) {
            self.log.push(packet);
        }
    }
    pub fn add_to_node(&mut self, packet: &[u8]) {
        if let Ok(packet) = Self::packet_to_node(packet) {
            self.log.push(packet);
        }
    }
    pub fn log(&self) -> &[PacketDirection] {
        &self.log
    }
}
