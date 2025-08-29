use std::fmt::Write;

use clap::Parser;
use clap::Subcommand;
use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;
use coap_message::MinimalWritableMessage;
use minicbor::Decoder;
use minicbor::Encoder;

use super::Command;
use super::CommandHandler;
use super::CommandRegistry;

/// This is an example on how to use cbor as payload for the coap request.
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
    /// Lists all attached sensors and actuators (this is the default)
    List,
    /// Read a value from a sensor
    Read { id: u8 },
    /// Write a 8 bit value into an actuator
    Write { id: u8, data: u8 },
}

pub struct Saul {
    location: String,
    buffer: String,
    payload: Vec<u8>,
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
            required_endpoints: vec!["/jelly/Saul".to_owned()],
        }
    }

    fn parse(cmd: &Command, args: String) -> Result<Box<dyn CommandHandler>, String> {
        let cli = SaulCli::try_parse_from(args.split_whitespace()).map_err(|e| e.to_string())?;
        Ok(Box::new(Self {
            location: cmd.required_endpoints[0].clone(),
            buffer: String::new(),
            payload: vec![],
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
                None | Some(SaulOperation::List) => encoder.array(1).unwrap().u8(0).unwrap().end(),
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
        self.payload = payload.to_vec();
        let mut out = String::new();
        let mut decoder = Decoder::new(payload);

        match self.cli.operation {
            None | Some(SaulOperation::List) => {
                out = decode_sensor_list_into_string(payload);
            }
            Some(SaulOperation::Read { id }) => {
                let data = decoder.map();
                match data {
                    // Dirty: assuming name, unit, value
                    Ok(Some(3)) => {
                        let _name_id = decoder.u8(); // Should be 0, see rfc8428#section-6
                        let name = decoder.str().unwrap();
                        let _unit_id = decoder.u8(); // 1
                        let unit = decoder.str().unwrap();
                        let _value_id = decoder.u8(); // 2
                        let _value_tag = decoder.tag().unwrap(); // decimal fractions (CBOR Tag 4)
                        let _value_array = decoder.array();
                        let e = decoder.i32().unwrap();
                        let m = f32::from(decoder.i16().unwrap());
                        let value: f32 = m * 10f32.powi(e);
                        let _ = writeln!(out, "{name}: {value:?} °{unit}");
                    }
                    // Dirty: assuming name, value
                    Ok(Some(2)) => {
                        let _name_id = decoder.u8(); // Should be 0, see rfc8428#section-6
                        let name = decoder.str().unwrap();
                        let _value_id = decoder.u8(); // 4, bool
                        let value = decoder.bool().unwrap();
                        let _ = writeln!(out, "{name}: {value}");
                    }
                    Ok(Some(_)) => {
                        let _ = writeln!(out, "SenML response type not implemented.");
                    }
                    Ok(None) => {
                        let _ = writeln!(
                            out,
                            "Malformed CBOR response, expected Map with exactly three elements."
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

    fn export(&self) -> Vec<u8> {
        self.payload.clone()
    }
}

fn decode_sensor_list_into_string(payload: &[u8]) -> String {
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
    out
}
