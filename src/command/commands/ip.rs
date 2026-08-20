use std::fmt::Write;
use std::write;

use clap::Parser;
use clap::Subcommand;
use coap_lite::RequestType as Method;
use coap_lite::{CoapRequest, Packet};
use coap_message::MinimalWritableMessage;
use minicbor::Decoder;
use minicbor::Encoder;
//use senml;

use super::Command;
use super::CommandHandler;
use super::CommandType;

/// This is an example on how to use cbor as payload for the coap request.
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
    })))
}

impl CommandHandler for Ifconfig {
    fn init(&mut self) -> CoapRequest<String> {
        let mut buffer: Vec<u8> = vec![];
        let mut encoder = Encoder::new(&mut buffer);
        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_path(&self.location);

        let request = if let Some(iface_id) = &self.cli.iface {
            let method = if let Some(operation) = &self.cli.operation {
                encoder
                    .array(2)
                    .unwrap()
                    .tag(minicbor::data::Tag::new(20))
                    .unwrap()
                    .str(iface_id)
                    .unwrap();
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
                            .tag(minicbor::data::Tag::new(309))
                            .unwrap()
                            .bool(true)
                            .unwrap();
                        Method::Patch
                    }
                    IfconfigOperation::Down => {
                        encoder
                            .tag(minicbor::data::Tag::new(309))
                            .unwrap()
                            .bool(false)
                            .unwrap();
                        Method::Patch
                    }
                    IfconfigOperation::Set { key: _, value: _ } => todo!(),
                }
            } else {
                encoder
                    .tag(minicbor::data::Tag::new(20))
                    .unwrap()
                    .str(iface_id)
                    .unwrap();

                Method::Get
            };
            request.set_method(method);
            request
                .message
                .set_content_format(coap_lite::ContentFormat::ApplicationCBOR);
            request.message.set_payload(&buffer).unwrap();

            request
        } else {
            request.set_method(Method::Get);
            request
        };

        request
    }

    fn handle(&mut self, response: &Packet) -> Option<CoapRequest<String>> {
        let resp_status = match response.header.code {
            coap_lite::MessageClass::Response(ref code) => code,
            _ => &coap_lite::ResponseType::UnKnown,
        };
        self.payload.clone_from(&response.payload);
        let mut out = String::new();

        if let Some(operation) = &self.cli.operation {
            match operation {
                IfconfigOperation::Add { addr: _ } | IfconfigOperation::Del { addr: _ } => {
                    if resp_status.is_error() {
                        let _ = writeln!(out, "Couldn't add/del ip address");
                    }
                }
                _ => (), // no op
            }
        } else {
            // Todo: React to the error specific
            if resp_status.is_error() {
                let _ = writeln!(out, "Couldn't list the interface(s): {resp_status:?}");
            } else {
                out = decode_netif_list_into_string(&self.payload);
            }
        }
        self.buffer = out;
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
        let last_i = self.data.len() - 1;
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
                panic!();
            };
            Self { addr, prefix }
        } else {
            panic!();
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
                .tag(minicbor::data::Tag::new(54))
                .unwrap()
                .array(2)
                .unwrap()
                .bytes(&addr_octs)
                .unwrap()
                .u8(self.prefix)
                .unwrap();
        } else {
            encoder
                .tag(minicbor::data::Tag::new(54))
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
        let mut me = Self {
            ..Default::default()
        };

        if decoder.probe().array().is_ok() {
            decoder.array().unwrap();
            while decoder.probe().tag().is_ok() {
                match decoder.tag().unwrap().as_u64() {
                    314 => me.phy = Some(decoder.u32().unwrap()),
                    315 => me.oqpsk_rate = Some(decoder.u32().unwrap()),
                    316 => me.mr_oqpsk_chips = Some(decoder.u32().unwrap()),
                    317 => me.mr_oqpsk_rate = Some(decoder.u32().unwrap()),
                    318 => me.mr_ofdm_option = Some(decoder.u32().unwrap()),
                    319 => me.mr_ofdm_mcs = Some(decoder.u32().unwrap()),
                    320 => me.mr_fsk_modulation_index = Some(decoder.u32().unwrap()),
                    321 => me.mr_fsk_modulation_order = Some(decoder.u32().unwrap()),
                    322 => me.mr_fsk_srate = Some(decoder.u32().unwrap()),
                    323 => me.mr_fsk_fec = Some(decoder.u32().unwrap()),
                    324 => me.channel_spacing = Some(decoder.u32().unwrap()),
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
    fn set(&mut self, tag: u64, flag: bool) {
        match tag {
            326 => self.promisc = Some(flag),
            327 => self.autoack = Some(flag),
            328 => self.ack_req = Some(flag),
            329 => self.preload = Some(flag),
            330 => self.rawmode = Some(flag),
            331 => self.mac_no_sleep = Some(flag),
            332 => self.csma = Some(flag),
            333 => self.autocca = Some(flag),
            334 => self.iq_invert = Some(flag),
            335 => self.rx_single = Some(flag),
            336 => self.chan_hop = Some(flag),
            337 => self.otaa = Some(flag),
            338 => self.rtr = Some(flag),
            339 => self.rtr_adv = Some(flag),
            340 => self.sixlo = Some(flag),
            341 => self.abr = Some(flag),
            342 => self.iphc = Some(flag),
            _ => (),
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
struct Iface {
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
}

impl Iface {
    fn from_cbor(decoder: &mut Decoder) -> Self {
        let mut me = Self {
            name: "NoName".to_string(),
            ipv6addr: vec![],
            wired: true,
            ..Default::default()
        };

        while decoder.probe().tag().is_ok() {
            let tag = decoder.tag().unwrap().as_u64();
            match tag {
                20 => {
                    me.name = decoder.str().unwrap().to_string();
                }
                48 => {
                    me.mac = Some(Eui64::new(
                        decoder
                            .bytes()
                            .unwrap()
                            .try_into()
                            .expect("Mac should be 8 bytes"),
                    ));
                }
                54 => {
                    me.ipv6addr.push(Ipv6AddrCidr::from_cbor(decoder));
                }
                302 => {
                    me.wired = decoder.bool().unwrap();
                }
                304 => {
                    me.channel = Some(decoder.u16().unwrap());
                }
                305 => {
                    me.channel_frequency = Some(decoder.u32().unwrap());
                }
                306 => {
                    me.channel_page = Some(decoder.u16().unwrap());
                }
                307 => {
                    me.network_id = Some(decoder.u16().unwrap());
                }
                308 => {
                    me.rssi = Some(decoder.i16().unwrap());
                }
                309 => {
                    me.link = Some(decoder.bool().unwrap());
                }
                310 => {
                    me.tx_power = Some(decoder.u16().unwrap());
                }
                311 => {
                    me.state = Some(decoder.u32().unwrap());
                }
                312 => {
                    me.retrans = Some(decoder.u8().unwrap());
                }
                313 => {
                    me.csma_retries = Some(decoder.u8().unwrap());
                }
                325 => me.ieee802154 = Some(Ieee802154::from_cbor(decoder)),
                326..=342 => me.netif_flags.set(tag, decoder.bool().unwrap()),
                343 => {
                    me.l2_pdu = Some(decoder.u16().unwrap());
                }
                344 => {
                    me.mtu = Some(decoder.u16().unwrap());
                }
                345 => {
                    me.hop_limit = Some(decoder.u8().unwrap());
                }
                _ => decoder.skip().unwrap(),
            }
        }
        me
    }
}

impl std::fmt::Display for Iface {
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

fn decode_netif_list_into_string(data: &[u8]) -> String {
    let mut out = String::new();
    let mut decoder = Decoder::new(data);

    decoder.array().unwrap();
    {
        while decoder.probe().array().is_ok() {
            decoder.array().unwrap();
            let _ = writeln!(out, "{}", Iface::from_cbor(&mut decoder));
            if let Ok(minicbor::data::Type::Break) = decoder.probe().datatype() {
                decoder.skip().unwrap();
            }
        }
    }
    out
}
