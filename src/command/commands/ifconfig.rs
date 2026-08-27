use std::fmt::Write;
use std::println;
use std::vec;
use std::write;

use clap::Parser;
use clap::Subcommand;
use coap_lite::RequestType as Method;
use coap_lite::{CoapRequest, Packet};
use coap_message::MinimalWritableMessage;
use minicbor::Decoder;
use minicbor::Encoder;

use super::Command;
use super::CommandHandler;
use super::CommandType;

const TAG_IEEE_MAC: u64 = 48; /* Choosen by IANA */
const TAG_IPV6: u64 = 54; /* Choosen by IANA */

const TAG_NETIF_NAME: u64 = 0;
const TAG_NETOPT_ADDRESS: u64 = 1;
const TAG_NETOPT_ADDRESS_LONG: u64 = 2;
const TAG_NETOPT_IS_WIRED: u64 = 3;

const TAG_NETOPT_IPV6_ADDR: u64 = 4;
const TAG_NETOPT_IPV6_GROUP: u64 = 5;

const TAG_NETOPT_CHANNEL: u64 = 6;
const TAG_NETOPT_CHANNEL_FREQUENCY: u64 = 7;
const TAG_NETOPT_CHANNEL_PAGE: u64 = 8;
const TAG_NETOPT_NID: u64 = 9;
const TAG_NETOPT_RSSI: u64 = 10;
const TAG_NETOPT_CCA_THRESHOLD: u64 = 11;
const TAG_NETOPT_LINK: u64 = 12;
const TAG_NETOPT_ACTIVE: u64 = 13;
const TAG_NETOPT_TX_POWER: u64 = 14;
const TAG_NETOPT_STATE: u64 = 15;
const TAG_NETOPT_RETRANS: u64 = 16;
const TAG_NETOPT_CSMA_RETRIES: u64 = 17;
const TAG_NETOPT_MAX_PDU_SIZE: u64 = 18;
const TAG_NETOPT_MAX_PDU_SIZE_IPV6: u64 = 19;
const TAG_NETOPT_HOP_LIMIT: u64 = 20;
const TAG_NETOPT_SRC_LEN: u64 = 21;

const TAG_FLAG_ARRAY: u64 = 22;
const TAG_NETOPT_PROMISCUOUSMODE: u64 = 0;
const TAG_NETOPT_AUTOACK: u64 = 1;
const TAG_NETOPT_ACK_REQ: u64 = 2;
const TAG_NETOPT_PRELOADING: u64 = 3;
const TAG_NETOPT_RAWMODE: u64 = 4;
const TAG_NETOPT_MAC_NO_SLEEP: u64 = 5;
const TAG_NETOPT_CSMA: u64 = 6;
const TAG_NETOPT_AUTOCCA: u64 = 7;
const TAG_NETOPT_IQ_INVERT: u64 = 8;
const TAG_NETOPT_SINGLE_RECEIVE: u64 = 9;
const TAG_NETOPT_CHANNEL_HOP: u64 = 10;
const TAG_NETOPT_OTAA: u64 = 11;
const TAG_NETOPT_IPV6_FORWARDING: u64 = 12;
const TAG_NETOPT_IPV6_SND_RTR_ADV: u64 = 13;
const TAG_NETOPT_6LO: u64 = 14;
const TAG_NETOPT_6LO_ABR: u64 = 15;
const TAG_NETOPT_6LO_IPHC: u64 = 16;

const TAG_IEEE802154_ARRAY: u64 = 23;
const TAG_NETOPT_IEEE802154_PHY: u64 = 0;
const TAG_NETOPT_OQPSK_RATE: u64 = 1;
const TAG_NETOPT_MR_OQPSK_CHIPS: u64 = 2;
const TAG_NETOPT_MR_OQPSK_RATE: u64 = 3;
const TAG_NETOPT_MR_OFDM_OPTION: u64 = 4;
const TAG_NETOPT_MR_OFDM_MCS: u64 = 5;
const TAG_NETOPT_MR_FSK_MODULATION_INDEX: u64 = 6;
const TAG_NETOPT_MR_FSK_MODULATION_ORDER: u64 = 7;
const TAG_NETOPT_MR_FSK_SRATE: u64 = 8;
const TAG_NETOPT_MR_FSK_FEC: u64 = 9;
const TAG_NETOPT_CHANNEL_SPACING: u64 = 10;

const TAG_LORA_ARRAY: u64 = 24;
const TAG_NETOPT_BANDWIDTH: u64 = 0;
const TAG_NETOPT_SPREADING_FACTOR: u64 = 1;
const TAG_NETOPT_CODING_RATE: u64 = 2;
const TAG_NETOPT_DEMOD_MARGIN: u64 = 3;
const TAG_NETOPT_NUM_GATEWAYS: u64 = 4;

#[derive(Parser, Debug)]
#[command(name = "Ifconfig")]
#[command(version = "1.0")]
#[command(disable_help_flag = false)]
#[command(about = "This is ifconfig over coap")]
pub struct IfconfigCli {
    iface: Option<String>,

    #[command(subcommand)]
    operation: Option<IfconfigOperation>,
}

#[derive(Subcommand, Debug)]
enum IfconfigOperation {
    Up,
    Down,
    Set {
        key: String,
        value: String,
    },
    /// Add an IPv6 address to the interface
    Add {
        addr: Ipv6AddrCidr,
    },
    /// Delete an IPv6 address from the interface
    Del {
        addr: Ipv6AddrCidr,
    },
}

struct Ifconfig {
    location: String,
    buffer: String,
    payload: Vec<u8>,
    finished: bool,
    displayable: bool,
    cli: IfconfigCli,
    ifaces: Vec<String>,
}

pub fn cmd() -> Command {
    Command {
        cmd: "Ifconfig".to_owned(),
        description: "ifconfig over coap".to_owned(),
        parse: |s, a| parse(s, a),
        required_endpoints: vec!["/jelly/netif".to_owned()],
    }
}

fn parse(cmd: &Command, args: &str) -> Result<CommandType, String> {
    let cli = IfconfigCli::try_parse_from(args.split_whitespace()).map_err(|e| e.to_string())?;
    if cli.operation.is_some() && cli.iface.is_none() {
        return Err("a subcommand can only be used when an interface is given".to_string());
    }
    Ok(CommandType::CoAP(Box::new(Ifconfig {
        location: cmd.required_endpoints[0].clone(),
        buffer: String::new(),
        payload: vec![],
        finished: false,
        displayable: false,
        cli,
        ifaces: vec![],
    })))
}

impl CommandHandler for Ifconfig {
    fn init(&mut self) -> CoapRequest<String> {
        let mut buffer: Vec<u8> = vec![];
        let mut encoder = Encoder::new(&mut buffer);
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_path(&self.location);

        if let Some(iface_id) = &self.cli.iface {
            request
                .message
                .add_option_str(coap_lite::CoapOption::UriQuery, &format!("name={iface_id}"))
                .unwrap();
            let method = if let Some(operation) = &self.cli.operation {
                encoder.array(1).unwrap();
                match operation {
                    IfconfigOperation::Add { addr } => {
                        addr.into_cbor_with_prefix(&mut encoder);
                        Method::Post
                    }
                    IfconfigOperation::Del { addr } => {
                        addr.into_cbor_without_prefix(&mut encoder);
                        Method::Patch
                    }
                    IfconfigOperation::Up => {
                        encoder
                            .tag(minicbor::data::Tag::new(TAG_NETOPT_LINK))
                            .unwrap()
                            .bool(true)
                            .unwrap();
                        Method::Patch
                    }
                    IfconfigOperation::Down => {
                        encoder
                            .tag(minicbor::data::Tag::new(TAG_NETOPT_LINK))
                            .unwrap()
                            .bool(false)
                            .unwrap();
                        Method::Patch
                    }
                    IfconfigOperation::Set { key, value } => {
                        match key.as_str() {
                            "addr_long" => {
                                let bytes = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
                                encoder
                                    .tag(minicbor::data::Tag::new(TAG_NETOPT_ADDRESS_LONG))
                                    .unwrap()
                                    .bytes(&bytes)
                                    .unwrap();
                            }
                            "nid" => {
                                let nid = 27;
                                encoder
                                    .tag(minicbor::data::Tag::new(TAG_NETOPT_NID))
                                    .unwrap()
                                    .u16(nid)
                                    .unwrap();
                            }
                            _ => todo!(),
                        }
                        Method::Patch
                    }
                }
            } else {
                Method::Get
            };
            request.set_method(method);
            request
                .message
                .set_content_format(coap_lite::ContentFormat::ApplicationCBOR);
            request.message.set_payload(&buffer).unwrap();
        } else {
            request.set_method(Method::Get);
        }

        request
    }

    fn handle(&mut self, response: &Packet) -> Option<CoapRequest<String>> {
        fn _ifaces_from_cbor_list(cbor: &[u8]) -> Vec<String> {
            let mut ret = vec![];
            let mut decoder = Decoder::new(cbor);
            if decoder.probe().array().is_ok() {
                decoder.array().unwrap();
                while decoder.probe().str().is_ok() {
                    ret.push(decoder.str().unwrap().to_string());
                }
            }
            ret
        }

        let resp_status = match response.header.code {
            coap_lite::MessageClass::Response(ref code) => code,
            _ => &coap_lite::ResponseType::UnKnown,
        };
        self.payload.clone_from(&response.payload);

        if let Some(operation) = &self.cli.operation {
            match operation {
                IfconfigOperation::Add { addr: _ } | IfconfigOperation::Del { addr: _ } => {
                    if resp_status.is_error() {
                        let _ = writeln!(self.buffer, "Couldn't add/del ip address");
                    }
                }
                _ => (), // no op
            }
        } else if self.cli.iface.is_some() {
            // Todo: React to the error specific
            if resp_status.is_error() {
                let _ = writeln!(
                    self.buffer,
                    "Couldn't list the interface(s): {resp_status:?}"
                );
            } else {
                let _ = writeln!(self.buffer, "{}", decode_netif_into_string(&self.payload));
            }
        } else {
            self.ifaces = _ifaces_from_cbor_list(&self.payload);
        }

        if let Some(iface) = self.ifaces.pop() {
            self.cli.iface = Some(iface.clone());
            let mut request: CoapRequest<String> = CoapRequest::new();
            request.set_path(&self.location);
            request
                .message
                .add_option_str(coap_lite::CoapOption::UriQuery, &format!("name={iface}"))
                .unwrap();
            request.set_method(Method::Get);
            return Some(request);
        }

        self.finished = true;
        self.displayable = true;
        None
    }

    fn want_display(&self) -> bool {
        self.displayable
    }

    fn is_finished(&self) -> bool {
        self.finished
    }

    fn display(&self, buffer: &mut String) {
        let _ = writeln!(buffer, "{}", self.buffer);
    }

    fn export(&self) -> Vec<u8> {
        self.payload.clone()
    }
}

#[derive(Default)]
struct Eui64 {
    data: [u8; 8],
    len: usize,
}

impl Eui64 {
    fn new(data: &[u8]) -> Self {
        let len = data.len();
        let mut me = Self::default();
        if len > 8 {
            me
        } else {
            me.data[..len].clone_from_slice(data);
            me.len = len;
            me
        }
    }
}

impl std::fmt::Display for Eui64 {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let last_i = self.len - 1;
        for i in 0..last_i {
            write!(out, "{:02X}:", self.data[i])?;
        }
        write!(out, "{:02X}", self.data[last_i])
    }
}

#[derive(Clone, Debug)]
struct Ipv6AddrCidr {
    addr: std::net::Ipv6Addr,
    prefix: u8,
}

impl std::fmt::Display for Ipv6AddrCidr {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        if self.addr.is_multicast() {
            write!(out, "{}", self.addr)
        } else {
            write!(out, "{}/{}", self.addr, self.prefix)
        }
    }
}

impl Ipv6AddrCidr {
    fn from_cbor(decoder: &mut Decoder) -> Self {
        // rfc9164#section-3.1.1 "Address Format"
        if decoder.probe().bytes().is_ok() {
            // rfc9164#name-ipv6 "to be encoded as a sixteen-byte byte string"
            let data: &[u8; 16] = decoder.bytes().unwrap().try_into().unwrap();
            let addr = std::net::Ipv6Addr::from(*data);
            Self {
                addr,
                prefix: if addr.is_unicast_link_local() {
                    64
                } else {
                    128
                },
            }
        } else if decoder.probe().array().is_ok() {
            decoder.array().unwrap();
            // rfc9164#section-3.1.3 "Interface Definition"
            let (addr, prefix) = if decoder.probe().bytes().is_ok() {
                let data: &[u8; 16] = decoder.bytes().unwrap().try_into().unwrap();
                let addr = std::net::Ipv6Addr::from(*data);
                let prefix = decoder.u8().unwrap_or(128);
                (addr, prefix)
            } else
            // rfc9164#section-3.1.2 "Prefix Format"
            if decoder.probe().u8().is_ok() {
                let prefix = decoder.u8().unwrap_or(128);
                let data: &[u8; 16] = decoder.bytes().unwrap().try_into().unwrap();
                (std::net::Ipv6Addr::from(*data), prefix)
            } else {
                todo!();
            };
            Self { addr, prefix }
        } else {
            todo!();
        }
    }

    fn into_cbor_with_prefix<W>(&self, encoder: &mut Encoder<W>)
    where
        W: minicbor::encode::Write<Error = core::convert::Infallible>,
    {
        self.into_cbor(encoder, true);
    }

    fn into_cbor_without_prefix<W>(&self, encoder: &mut Encoder<W>)
    where
        W: minicbor::encode::Write<Error = core::convert::Infallible>,
    {
        self.into_cbor(encoder, false);
    }

    fn into_cbor<W>(&self, encoder: &mut Encoder<W>, with_prefix: bool)
    where
        W: minicbor::encode::Write<Error = core::convert::Infallible>,
    {
        let addr_octs = self.addr.octets();
        if with_prefix {
            encoder
                .tag(minicbor::data::Tag::new(TAG_IPV6))
                .unwrap()
                .array(2)
                .unwrap()
                .bytes(&addr_octs)
                .unwrap()
                .u8(self.prefix)
                .unwrap();
        } else {
            encoder
                .tag(minicbor::data::Tag::new(TAG_IPV6))
                .unwrap()
                .bytes(&addr_octs)
                .unwrap();
        }
    }
}

impl std::str::FromStr for Ipv6AddrCidr {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let mut splitted = s.split('/');
        if let Some(addr) = splitted.next() {
            let addr = std::net::Ipv6Addr::from_str(addr).map_err(|e| e.to_string())?;
            let prefix = if let Some(prefix) = splitted.next() {
                u8::from_str(prefix).map_err(|e| e.to_string())?
            } else {
                128
            };
            Ok(Self { addr, prefix })
        } else {
            Err("No ipv6 addr provided".to_string())
        }
    }
}

#[derive(Default)]
struct Lora {
    bandwidth: Option<u8>,
    spreading_factor: Option<u8>,
    coding_rate: Option<u8>,
    demod_margin: Option<u8>,
    num_gateways: Option<u8>,
}

impl Lora {
    fn from_cbor(decoder: &mut Decoder) -> Self {
        let mut me = Self::default();

        if decoder.probe().array().is_ok() {
            decoder.array().unwrap();
            while decoder.probe().u64().is_ok() {
                match decoder.u64().unwrap() {
                    TAG_NETOPT_BANDWIDTH => me.bandwidth = Some(decoder.u8().unwrap()),
                    TAG_NETOPT_SPREADING_FACTOR => {
                        me.spreading_factor = Some(decoder.u8().unwrap())
                    }
                    TAG_NETOPT_CODING_RATE => me.coding_rate = Some(decoder.u8().unwrap()),
                    TAG_NETOPT_DEMOD_MARGIN => me.demod_margin = Some(decoder.u8().unwrap()),
                    TAG_NETOPT_NUM_GATEWAYS => me.num_gateways = Some(decoder.u8().unwrap()),
                    _ => decoder.skip().unwrap(),
                }
            }
            // Skip array end
            decoder.skip().unwrap();
        }

        me
    }
}

impl std::fmt::Display for Lora {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        if let Some(bandwidth) = &self.bandwidth {
            if *bandwidth < 3 {
                let _netopt_bandwidth_str: [&str; 3] = ["125", "250", "500"];
                write!(
                    out,
                    "  BW: {:} kHz",
                    _netopt_bandwidth_str[*bandwidth as usize]
                )?;
            }
        }
        if let Some(spreading_factor) = &self.spreading_factor {
            write!(out, "  spreading_factor: {spreading_factor}")?;
        }
        if let Some(coding_rate) = &self.coding_rate {
            if *coding_rate < 4 {
                let _netopt_coding_rate_str: [&str; 4] = ["4/5", "4/6", "4/7", "4/8"];
                write!(
                    out,
                    "  CR: {:}",
                    _netopt_coding_rate_str[*coding_rate as usize]
                )?;
            }
        }
        if let Some(demod_margin) = &self.demod_margin {
            write!(out, "  demod_margin: {demod_margin}")?;
        }
        if let Some(num_gateways) = &self.num_gateways {
            write!(out, "  num_gateways: {num_gateways}")?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct Ieee802154 {
    phy: Option<u32>,
    oqpsk_rate: Option<u32>,
    mr_oqpsk_chips: Option<u32>,
    mr_oqpsk_rate: Option<u32>,
    mr_ofdm_option: Option<u32>,
    mr_ofdm_mcs: Option<u32>,
    mr_fsk_modulation_index: Option<u32>,
    mr_fsk_modulation_order: Option<u32>,
    mr_fsk_srate: Option<u32>,
    mr_fsk_fec: Option<u32>,
    channel_spacing: Option<u32>,
}

impl Ieee802154 {
    fn from_cbor(decoder: &mut Decoder) -> Self {
        let mut me = Self::default();

        if decoder.probe().map().is_ok() {
            decoder.map().unwrap();
            while decoder.probe().u64().is_ok() {
                match decoder.u64().unwrap() {
                    TAG_NETOPT_IEEE802154_PHY => me.phy = Some(decoder.u32().unwrap()),
                    TAG_NETOPT_OQPSK_RATE => me.oqpsk_rate = Some(decoder.u32().unwrap()),
                    TAG_NETOPT_MR_OQPSK_CHIPS => me.mr_oqpsk_chips = Some(decoder.u32().unwrap()),
                    TAG_NETOPT_MR_OQPSK_RATE => me.mr_oqpsk_rate = Some(decoder.u32().unwrap()),
                    TAG_NETOPT_MR_OFDM_OPTION => me.mr_ofdm_option = Some(decoder.u32().unwrap()),
                    TAG_NETOPT_MR_OFDM_MCS => me.mr_ofdm_mcs = Some(decoder.u32().unwrap()),
                    TAG_NETOPT_MR_FSK_MODULATION_INDEX => {
                        me.mr_fsk_modulation_index = Some(decoder.u32().unwrap())
                    }
                    TAG_NETOPT_MR_FSK_MODULATION_ORDER => {
                        me.mr_fsk_modulation_order = Some(decoder.u32().unwrap())
                    }
                    TAG_NETOPT_MR_FSK_SRATE => me.mr_fsk_srate = Some(decoder.u32().unwrap()),
                    TAG_NETOPT_MR_FSK_FEC => me.mr_fsk_fec = Some(decoder.u32().unwrap()),
                    TAG_NETOPT_CHANNEL_SPACING => me.channel_spacing = Some(decoder.u32().unwrap()),
                    _ => decoder.skip().unwrap(),
                }
            }
            // Skip array end
            decoder.skip().unwrap();
        }

        me
    }
}

impl std::fmt::Display for Ieee802154 {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        if let Some(phy) = &self.phy {
            if *phy < 7 {
                let _netopt_ieee802154_phy_str: [&str; 7] = [
                    "DISABLED",
                    "BPSK",
                    "ASK",
                    "O-QPSK",
                    "MR-O-QPSK",
                    "MR-OFDM",
                    "MR-FSK",
                ];
                write!(out, "  PHY: {:}", _netopt_ieee802154_phy_str[*phy as usize])?;
            }
        }
        if let Some(oqpsk_rate) = &self.oqpsk_rate {
            write!(out, "  oqpsk_rate: {oqpsk_rate}")?;
        }
        if let Some(mr_oqpsk_chips) = &self.mr_oqpsk_chips {
            write!(out, "  mr_oqpsk_chips: {mr_oqpsk_chips}")?;
        }
        if let Some(mr_oqpsk_rate) = &self.mr_oqpsk_rate {
            write!(out, "  mr_oqpsk_rate: {mr_oqpsk_rate}")?;
        }
        if let Some(mr_ofdm_option) = &self.mr_ofdm_option {
            write!(out, "  mr_ofdm_option: {mr_ofdm_option}")?;
        }
        if let Some(mr_ofdm_mcs) = &self.mr_ofdm_mcs {
            write!(out, "  mr_ofdm_mcs: {mr_ofdm_mcs}")?;
        }
        if let Some(mr_fsk_modulation_index) = &self.mr_fsk_modulation_index {
            writeln!(out, "  mr_fsk_modulation_index: {mr_fsk_modulation_index}")?;
        }
        if let Some(mr_fsk_modulation_order) = &self.mr_fsk_modulation_order {
            write!(
                out,
                "  mr_fsk_modulation_order: {mr_fsk_modulation_order}-FSK"
            )?;
        }
        if let Some(mr_fsk_srate) = &self.mr_fsk_srate {
            write!(out, "  mr_fsk_srate: {mr_fsk_srate} kHz")?;
        }
        if let Some(mr_fsk_fec) = &self.mr_fsk_fec {
            write!(out, "  mr_fsk_fec: {mr_fsk_fec}")?;
        }
        if let Some(channel_spacing) = &self.channel_spacing {
            write!(out, "  channel_spacing: {channel_spacing} kHz")?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct NetifFlags {
    promisc: Option<bool>,
    autoack: Option<bool>,
    ack_req: Option<bool>,
    preload: Option<bool>,
    rawmode: Option<bool>,
    mac_no_sleep: Option<bool>,
    csma: Option<bool>,
    autocca: Option<bool>,
    iq_invert: Option<bool>,
    rx_single: Option<bool>,
    chan_hop: Option<bool>,
    otaa: Option<bool>,
    rtr: Option<bool>,
    rtr_adv: Option<bool>,
    sixlo: Option<bool>,
    abr: Option<bool>,
    iphc: Option<bool>,
}

impl NetifFlags {
    fn set(&mut self, decoder: &mut Decoder) {
        if decoder.probe().array().is_ok() {
            decoder.array().unwrap();
            while decoder.probe().u64().is_ok() {
                let tag = decoder.u64().unwrap();
                let flag = decoder.bool().unwrap();
                match tag {
                    TAG_NETOPT_PROMISCUOUSMODE => self.promisc = Some(flag),
                    TAG_NETOPT_AUTOACK => self.autoack = Some(flag),
                    TAG_NETOPT_ACK_REQ => self.ack_req = Some(flag),
                    TAG_NETOPT_PRELOADING => self.preload = Some(flag),
                    TAG_NETOPT_RAWMODE => self.rawmode = Some(flag),
                    TAG_NETOPT_MAC_NO_SLEEP => self.mac_no_sleep = Some(flag),
                    TAG_NETOPT_CSMA => self.csma = Some(flag),
                    TAG_NETOPT_AUTOCCA => self.autocca = Some(flag),
                    TAG_NETOPT_IQ_INVERT => self.iq_invert = Some(flag),
                    TAG_NETOPT_SINGLE_RECEIVE => self.rx_single = Some(flag),
                    TAG_NETOPT_CHANNEL_HOP => self.chan_hop = Some(flag),
                    TAG_NETOPT_OTAA => self.otaa = Some(flag),
                    TAG_NETOPT_IPV6_FORWARDING => self.rtr = Some(flag),
                    TAG_NETOPT_IPV6_SND_RTR_ADV => self.rtr_adv = Some(flag),
                    TAG_NETOPT_6LO => self.sixlo = Some(flag),
                    TAG_NETOPT_6LO_ABR => self.abr = Some(flag),
                    TAG_NETOPT_6LO_IPHC => self.iphc = Some(flag),
                    _ => (),
                }
            }
            if let Ok(minicbor::data::Type::Break) = decoder.probe().datatype() {
                decoder.skip().unwrap();
            }
        }
    }
}

impl std::fmt::Display for NetifFlags {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        if self.promisc.is_some_and(|x| x) {
            write!(out, "  PROMISC")?;
        }
        if self.autoack.is_some_and(|x| x) {
            write!(out, "  AUTOACK")?;
        }
        if self.ack_req.is_some_and(|x| x) {
            write!(out, "  ACK_REQ")?;
        }
        if self.preload.is_some_and(|x| x) {
            write!(out, "  PRELOAD")?;
        }
        if self.rawmode.is_some_and(|x| x) {
            write!(out, "  RAWMODE")?;
        }
        if self.mac_no_sleep.is_some_and(|x| x) {
            write!(out, "  MAC_NO_SLEEP")?;
        }
        if self.csma.is_some_and(|x| x) {
            write!(out, "  CSMA")?;
        }
        if self.autocca.is_some_and(|x| x) {
            write!(out, "  AUTOCCA")?;
        }
        if self.iq_invert.is_some_and(|x| x) {
            write!(out, "  IQ_INVERT")?;
        }
        if self.rx_single.is_some_and(|x| x) {
            write!(out, "  RX_SINGLE")?;
        }
        if self.chan_hop.is_some_and(|x| x) {
            write!(out, "  CHAN_HOP")?;
        }
        if self.otaa.is_some_and(|x| x) {
            write!(out, "  OTAA")?;
        }
        if self.rtr.is_some_and(|x| x) {
            write!(out, "  RTR")?;
        }
        if self.rtr_adv.is_some_and(|x| x) {
            write!(out, "  RTR_ADV")?;
        }
        if self.sixlo.is_some_and(|x| x) {
            write!(out, "  6LO")?;
        }
        if self.abr.is_some_and(|x| x) {
            write!(out, "  ABR")?;
        }
        if self.iphc.is_some_and(|x| x) {
            write!(out, "  IPHC")?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct Netif {
    name: String,
    mac: Option<Eui64>,
    ipv6addr: Vec<Ipv6AddrCidr>,
    wired: bool,
    channel: Option<u16>,
    channel_frequency: Option<u32>,
    channel_page: Option<u16>,
    network_id: Option<u16>,
    rssi: Option<i16>,
    link: Option<bool>,
    tx_power: Option<u16>,
    state: Option<u32>,
    retrans: Option<u8>,
    csma_retries: Option<u8>,
    l2_pdu: Option<u16>,
    mtu: Option<u16>,
    hop_limit: Option<u8>,
    ieee802154: Option<Ieee802154>,
    netif_flags: NetifFlags,
    lora: Option<Lora>,
}

impl Netif {
    fn from_cbor(decoder: &mut Decoder) -> Self {
        let mut me = Self {
            name: "NoName".to_string(),
            ipv6addr: vec![],
            wired: true,
            ..Default::default()
        };

        while decoder.probe().u64().is_ok() {
            let tag = decoder.u64().unwrap();
            match tag {
                TAG_NETIF_NAME => {
                    me.name = decoder.str().unwrap().to_string();
                }
                TAG_NETOPT_ADDRESS_LONG => {
                    if decoder.tag().is_ok_and(|x| x.as_u64() == TAG_IEEE_MAC) {
                        me.mac = Some(Eui64::new(decoder.bytes().unwrap().try_into().unwrap()));
                    }
                }
                TAG_NETOPT_IPV6_ADDR | TAG_NETOPT_IPV6_GROUP => {
                    if decoder.probe().array().is_ok() {
                        decoder.array().unwrap();
                        while decoder.probe().tag().is_ok_and(|x| x.as_u64() == TAG_IPV6) {
                            decoder.tag().unwrap();
                            me.ipv6addr.push(Ipv6AddrCidr::from_cbor(decoder));
                        }
                    }
                    if let Ok(minicbor::data::Type::Break) = decoder.probe().datatype() {
                        decoder.skip().unwrap();
                    }
                }
                TAG_NETOPT_IS_WIRED => {
                    me.wired = decoder.bool().unwrap();
                }
                TAG_NETOPT_CHANNEL => {
                    me.channel = Some(decoder.u16().unwrap());
                }
                TAG_NETOPT_CHANNEL_FREQUENCY => {
                    me.channel_frequency = Some(decoder.u32().unwrap());
                }
                TAG_NETOPT_CHANNEL_PAGE => {
                    me.channel_page = Some(decoder.u16().unwrap());
                }
                TAG_NETOPT_NID => {
                    me.network_id = Some(decoder.u16().unwrap());
                }
                TAG_NETOPT_RSSI => {
                    me.rssi = Some(decoder.i16().unwrap());
                }
                TAG_NETOPT_LINK => {
                    me.link = Some(decoder.bool().unwrap());
                }
                TAG_NETOPT_TX_POWER => {
                    me.tx_power = Some(decoder.u16().unwrap());
                }
                TAG_NETOPT_STATE => {
                    me.state = Some(decoder.u32().unwrap());
                }
                TAG_NETOPT_RETRANS => {
                    me.retrans = Some(decoder.u8().unwrap());
                }
                TAG_NETOPT_CSMA_RETRIES => {
                    me.csma_retries = Some(decoder.u8().unwrap());
                }
                TAG_IEEE802154_ARRAY => me.ieee802154 = Some(Ieee802154::from_cbor(decoder)),
                TAG_FLAG_ARRAY => me.netif_flags.set(decoder),
                TAG_NETOPT_MAX_PDU_SIZE => {
                    me.l2_pdu = Some(decoder.u16().unwrap());
                }
                TAG_NETOPT_MAX_PDU_SIZE_IPV6 => {
                    me.mtu = Some(decoder.u16().unwrap());
                }
                TAG_NETOPT_HOP_LIMIT => {
                    me.hop_limit = Some(decoder.u8().unwrap());
                }
                TAG_LORA_ARRAY => {
                    me.lora = Some(Lora::from_cbor(decoder));
                }
                _ => decoder.skip().unwrap(),
            }
        }
        me
    }
}

impl std::fmt::Display for Netif {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let _ = writeln!(out, "Iface {}", self.name);
        if let Some(mac) = &self.mac {
            writeln!(out, "  HWaddr: {mac}")?;
        }
        if let Some(channel) = &self.channel {
            write!(out, "  Channel: {channel}")?;
        }
        if let Some(channel_frequency) = &self.channel_frequency {
            write!(out, "  Frequency: {channel_frequency}")?;
        }
        if let Some(channel_page) = &self.channel_page {
            write!(out, "  Page: {channel_page}")?;
        }
        if let Some(network_id) = &self.network_id {
            write!(out, "  NID: 0x{network_id:x}")?;
        }
        if let Some(rssi) = &self.rssi {
            write!(out, "  RSSI: {rssi}")?;
        }
        if let Some(ieee802154) = &self.ieee802154 {
            write!(out, "{ieee802154}")?;
        }

        writeln!(out, "")?;

        if let Some(lora) = &self.lora {
            writeln!(out, "{lora}")?;
        }

        if let Some(link) = &self.link {
            write!(out, "  Link: {link}")?;
        }
        if let Some(tx_power) = &self.tx_power {
            write!(out, "  TX-Power: {tx_power}")?;
        }
        if let Some(state) = &self.state {
            let _netopt_state_str: [&str; 7] =
                ["OFF", "SLEEP", "IDLE", "RX", "TX", "RESET", "STANDBY"];
            if *state < 7 {
                write!(out, "  State: {:}", _netopt_state_str[*state as usize])?;
            } else {
                write!(out, "  State: {state}")?;
            }
        }

        writeln!(out, "")?;

        write!(out, "{:}", self.netif_flags)?;
        if let Some(l2_pdu) = &self.l2_pdu {
            write!(out, "  L2-PDU:{l2_pdu}")?;
        }
        if let Some(mtu) = &self.mtu {
            write!(out, "  MTU:{mtu}")?;
        }
        if let Some(hop_limit) = &self.hop_limit {
            write!(out, "  HL:{hop_limit}")?;
        }

        if let Some(retrans) = &self.retrans {
            write!(out, "  Retransmission: {retrans}")?;
        }
        if let Some(csma_retries) = &self.csma_retries {
            write!(out, "  CSMA: {csma_retries}")?;
        }

        writeln!(out, "")?;

        if self.wired {
            writeln!(out, "  Link type: wired")?;
        } else {
            writeln!(out, "  Link type: wireless")?;
        }
        for ip in &self.ipv6addr {
            if ip.addr.is_multicast() {
                writeln!(out, "  inet6 group: {ip}")?;
            } else {
                writeln!(out, "  inet6 addr: {ip}")?;
            }
        }
        Ok(())
    }
}

fn decode_netif_into_string(data: &[u8]) -> String {
    let mut out = String::new();
    let mut decoder = Decoder::new(data);

    if decoder.probe().map().is_ok() {
        decoder.map().unwrap();
        let _ = writeln!(out, "{}", Netif::from_cbor(&mut decoder));
        if let Ok(minicbor::data::Type::Break) = decoder.probe().datatype() {
            decoder.skip().unwrap();
        }
    }
    out
}
