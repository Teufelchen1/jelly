use std::fmt::Write;

use clap::Parser;
use clap::Subcommand;
use minicbor::Decoder;
use minicbor::Encoder;

#[derive(Parser, Debug)]
#[command(name = "SampleCommand")]
#[command(version = "1.0")]
#[command(disable_help_flag = false)]
#[command(about = "This is an example command")]
pub struct SampleCommand {
    #[arg(long)]
    caps: bool,
    #[arg(long, default_value_t = 1)]
    repeats: usize,
}

pub fn sample_command_display(payload: Vec<u8>) -> String {
    let buffer = String::from_utf8_lossy(&payload);
    buffer.replace(" ", "\n")
}

pub fn sample_command_handler(args: String) -> Result<Vec<u8>, String> {
    let cmd = SampleCommand::try_parse_from(args.split_whitespace()).map_err(|e| e.to_string())?;

    let mut buffer: [u8; 4] = [0; 4];
    let mut encoder = Encoder::new(&mut buffer[..]);

    let _ = encoder
        .array(2)
        .unwrap()
        .bool(cmd.caps)
        .unwrap()
        .u8(cmd.repeats.try_into().unwrap())
        .unwrap()
        .end();
    Ok(buffer.to_vec())
}

#[derive(Parser, Debug)]
#[command(name = "Saul")]
#[command(version = "1.0")]
#[command(disable_help_flag = false)]
#[command(about = "This is saul over coap")]
pub struct Saul {
    #[command(subcommand)]
    operation: Option<SaulOperation>,
}

#[derive(Subcommand, Debug)]
enum SaulOperation {
    Read { id: u8 },
    Write { id: u8, data: u8 },
}

pub fn saul_display(payload: Vec<u8>) -> String {
    const SAUL_CAT_MASK: u8 = 0xc0u8;
    /**< Bitmask to obtain the category ID */
    const SAUL_ID_MASK: u8 = 0x3fu8;
    /**< Bitmask to obtain the intra-category ID */

    const SAUL_CAT_UNDEF: u8 = 0x00;
    /**< device class undefined */
    const SAUL_CAT_ACT: u8 = 0x40;
    /**< Actuator device class */
    const SAUL_CAT_SENSE: u8 = 0x80;
    /**< Sensor device class */

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

    let mut decoder = Decoder::new(&payload);
    decoder.array().unwrap();
    match decoder.u8().unwrap() {
        // List
        0 => {
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
                let _ = write!(out, "{id}, {class_name}, {name}\n");
            }
        }
        // read
        1 => {
            let data = decoder.map();
            match data {
                Ok(_data) => {
                    let _ = write!(
                        out,
                        "{}\n",
                        cbor_edn::StandaloneItem::from_cbor(&payload[2..]).map_or_else(
                            |e| format!("Parsing error {e}, content {payload:02x?}\n"),
                            |c| c.serialize(),
                        )
                    );
                }
                Err(_) => {
                    let _ = write!(out, "No device with this id found\n");
                }
            }
        }
        // write
        2 => {}
        _ => {
            todo!();
        }
    }
    out
}

pub fn saul_handler(args: String) -> Result<Vec<u8>, String> {
    let cmd = Saul::try_parse_from(args.split_whitespace()).map_err(|e| e.to_string())?;

    let mut buffer: [u8; 12] = [0; 12];
    let mut encoder = Encoder::new(&mut buffer[..]);

    let _subcommand = {
        match cmd.operation {
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

    Ok(buffer.to_vec())
}
