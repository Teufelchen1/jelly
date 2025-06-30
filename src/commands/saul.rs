use std::fmt::Write;

use clap::Parser;
use clap::Subcommand;
use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;
use coap_message::MinimalWritableMessage;
use minicbor::Decoder;
use minicbor::Encoder;

use crate::commands::Command;
use crate::commands::CommandHandler;
use crate::commands::CommandRegistry;

#[derive(Parser, Debug)]
#[command(name = "Saul")]
#[command(version = "1.0")]
#[command(disable_help_flag = false)]
#[command(about = "This is saul over coap")]
pub struct SaulCli {
    #[command(subcommand)]
    operation: Option<SaulOperation>,
}

#[derive(Subcommand, Debug)]
enum SaulOperation {
    Read { id: u8 },
    Write { id: u8, data: u8 },
}

pub struct Saul {
    location: String,
    buffer: String,
    finished: bool,
    displayable: bool,
    cli: SaulCli,
}

impl CommandRegistry for Saul {
    fn cmd() -> Command {
        Command {
            cmd: "Saul".to_owned(),
            description: "Saul over coap".to_owned(),
            parse: |s, a| Self::parse(s, a),
            required_endpoints: vec!["/Saul".to_owned()],
        }
    }

    fn parse(cmd: &Command, args: String) -> Result<Box<dyn CommandHandler>, String> {
        let cli = SaulCli::try_parse_from(args.split_whitespace()).map_err(|e| e.to_string())?;
        Ok(Box::new(Self {
            location: cmd.required_endpoints[0].clone(),
            buffer: String::new(),
            finished: false,
            displayable: false,
            cli,
        }))
    }
}

impl CommandHandler for Saul {
    fn init(&mut self) -> CoapRequest<String> {
        let mut buffer: [u8; 12] = [0; 12];
        let mut encoder = Encoder::new(&mut buffer[..]);

        let _subcommand = {
            match self.cli.operation {
                None => encoder.array(1).unwrap().u8(0).unwrap().end(),
                Some(SaulOperation::Read { id }) => encoder
                    .array(2)
                    .unwrap()
                    .u8(1)
                    .unwrap()
                    .u8(id)
                    .unwrap()
                    .end(),
                Some(SaulOperation::Write { id, data }) => encoder
                    .array(3)
                    .unwrap()
                    .u8(2)
                    .unwrap()
                    .u8(id)
                    .unwrap()
                    .u8(data)
                    .unwrap()
                    .end(),
            }
        };

        let mut request: CoapRequest<String> = CoapRequest::new();
        request.set_method(Method::Post);
        request.set_path(&self.location);
        request
            .message
            .set_content_format(coap_lite::ContentFormat::ApplicationCBOR);
        request.message.set_payload(&buffer).unwrap();
        request
    }

    fn handle(&mut self, payload: &[u8]) -> Option<CoapRequest<String>> {
        /**< Bitmask to obtain the category ID */
        const SAUL_CAT_MASK: u8 = 0xc0u8;
        /**< Bitmask to obtain the intra-category ID */
        const SAUL_ID_MASK: u8 = 0x3fu8;
        /**< device class undefined */
        const SAUL_CAT_UNDEF: u8 = 0x00;
        /**< Actuator device class */
        const SAUL_CAT_ACT: u8 = 0x40;
        /**< Sensor device class */
        const SAUL_CAT_SENSE: u8 = 0x80;

        const SENSE_NAME: [&str; 29] = [
            "SAUL_SENSE_ID_ANY",
            "SAUL_SENSE_ID_BTN",
            "SAUL_SENSE_ID_TEMP",
            "SAUL_SENSE_ID_HUM",
            "SAUL_SENSE_ID_LIGHT",
            "SAUL_SENSE_ID_ACCEL",
            "SAUL_SENSE_ID_MAG",
            "SAUL_SENSE_ID_GYRO",
            "SAUL_SENSE_ID_COLOR",
            "SAUL_SENSE_ID_PRESS",
            "SAUL_SENSE_ID_ANALOG",
            "SAUL_SENSE_ID_UV",
            "SAUL_SENSE_ID_OBJTEMP",
            "SAUL_SENSE_ID_COUNT",
            "SAUL_SENSE_ID_DISTANCE",
            "SAUL_SENSE_ID_CO2",
            "SAUL_SENSE_ID_TVOC",
            "SAUL_SENSE_ID_GAS",
            "SAUL_SENSE_ID_OCCUP",
            "SAUL_SENSE_ID_PROXIMITY",
            "SAUL_SENSE_ID_RSSI",
            "SAUL_SENSE_ID_CHARGE",
            "SAUL_SENSE_ID_CURRENT",
            "SAUL_SENSE_ID_PM",
            "SAUL_SENSE_ID_CAPACITANCE",
            "SAUL_SENSE_ID_VOLTAGE",
            "SAUL_SENSE_ID_PH",
            "SAUL_SENSE_ID_POWER",
            "SAUL_SENSE_ID_SIZE",
        ];

        const ACT_NAME: [&str; 7] = [
            "SAUL_ACT_ID_ANY",
            "SAUL_ACT_ID_LED_RGB",
            "SAUL_ACT_ID_SERVO",
            "SAUL_ACT_ID_MOTOR",
            "SAUL_ACT_ID_SWITCH",
            "SAUL_ACT_ID_DIMMER",
            "SAUL_ACT_NUMOF",
        ];

        let mut out = String::new();

        let mut decoder = Decoder::new(payload);

        match self.cli.operation {
            None => {
                decoder.array().unwrap();
                while decoder.probe().array().is_ok() {
                    decoder.array().unwrap();
                    let id = decoder.u8().unwrap();
                    let class = decoder.u8().unwrap();
                    let name = decoder.str().unwrap();

                    let class_name = match class & SAUL_CAT_MASK {
                        SAUL_CAT_UNDEF => "UNDEFINED",
                        SAUL_CAT_ACT => ACT_NAME[(class & SAUL_ID_MASK) as usize],
                        SAUL_CAT_SENSE => SENSE_NAME[(class & SAUL_ID_MASK) as usize],
                        _ => todo!(),
                    };
                    let _ = writeln!(out, "{id}, {class_name}, {name}");
                }
            }
            Some(SaulOperation::Read { id }) => {
                let data = decoder.map();
                match data {
                    Ok(_data) => {
                        let _ = writeln!(
                            out,
                            "{}",
                            cbor_edn::StandaloneItem::from_cbor(payload).map_or_else(
                                |e| format!("Parsing error {e}, content {payload:02x?}"),
                                |c| c.serialize(),
                            )
                        );
                    }
                    Err(_) => {
                        let _ = writeln!(out, "No device with this id({id}) found.");
                    }
                }
            }
            Some(SaulOperation::Write { id: _, data: _ }) => {}
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
}
