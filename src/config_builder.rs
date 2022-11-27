//! Application configuration builder
use clap::{ArgAction, Parser};
use csv::StringRecord;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{stdin, Read};
use std::str::FromStr;

/// Expected final form of a csv record, ready to be assimilated as a key: value of a hash.
type CsvRecord = (bluer::Address, String);

/// Relations between the BLE address of a sensor and its name.
type SensorsMapping = HashMap<bluer::Address, String>;

/// Try to handle a record.
fn handle_record(
    result_record: Result<StringRecord, csv::Error>,
) -> Result<CsvRecord, Box<dyn Error>> {
    match result_record {
        Ok(record) => {
            if let (Some(addr), Some(name)) = (record.get(0), record.get(1)) {
                let addr = bluer::Address::from_str(addr)?;
                Ok((addr, name.to_owned()))
            } else {
                Err(format!("Cannot recognize two fields in the record: {:?}", &record).into())
            }
        }
        Err(e) => Err(e.into()),
    }
}

/// Get the mapping according to the cli arguments indications (stdin or file).
fn get_mapping_from_input(path: &Option<String>) -> Result<SensorsMapping, Box<dyn Error>> {
    // Read either from stdin or from the path given in the cli arguments
    let input_reader: Box<dyn Read> = match path {
        None => {
            println!("Waiting for sensors mapping on stdin...");
            Box::new(stdin())
        }
        Some(path) => {
            let fh = File::open(path)?;
            Box::new(fh)
        }
    };

    csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(input_reader)
        .into_records()
        .map(handle_record)
        .collect()
}

/// Cli allowed arguments.
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None,disable_help_flag(true))]
struct Cli {
    #[arg(
        short,
        long,
        default_value = "localhost",
        help = "Influxdb hostname or IP address"
    )]
    host: String,
    #[arg(short, long, default_value_t = 8086, help = "Influxdb port")]
    port: u16,
    #[arg(short, long, action=ArgAction::SetTrue, help="TLS connection flag (present/absent)")]
    tls: bool,
    #[arg(
        short,
        long,
        default_value = "weather_data",
        help = "Influxdb database name"
    )]
    database: String,
    #[arg(
        short,
        long,
        default_value = "weather_meas",
        help = "Influxdb measurement name"
    )]
    measurement: String,
    #[arg(
        short,
        long,
        help = "CSV file containing the sensors mapping (stdin if absent)"
    )]
    sensors: Option<String>,
    #[arg(short, long, action=ArgAction::SetTrue, help="Verbosity flag")]
    verbose: bool,
    #[arg(short='n', long="dry-run", action=ArgAction::SetTrue, help="Dry run: listen, but do not insert in database")]
    dry_run: bool,
    #[arg(long, action=ArgAction::Help, help="Print the help")]
    help: Option<bool>,
}

impl Cli {
    /// Build a correct connection string from the input arguments.
    fn get_influx_conn_string(&self) -> String {
        format!(
            "http{}://{}:{}",
            if self.tls { "s" } else { "" },
            self.host,
            self.port
        )
    }
}

/// Configuration of the application.
#[derive(Debug)]
pub struct AppConfig {
    /// InfluxDB connection string to the InfluxDB server.
    pub influx_conn: String,
    /// InfluxDB database name.
    pub influx_database: String,
    /// InfluxDB measurement name.
    pub influx_measurement: String,
    /// Mapping between the addresses of the sensors and their locations.
    pub sensors_names: SensorsMapping,
    /// Be verbose.
    pub be_verbose: bool,
    /// Dry run, do not run insert queries.
    pub dry_run: bool,
}

impl AppConfig {
    /// Build and get the configuration from the cli arguments
    /// and/or the sensors file or stdin.
    pub fn get_from_cli_inputs() -> Result<Self, Box<dyn Error>> {
        let cli_args = Cli::parse();
        let app_config = AppConfig {
            influx_conn: cli_args.get_influx_conn_string(),
            influx_database: cli_args.database,
            influx_measurement: cli_args.measurement,
            sensors_names: get_mapping_from_input(&cli_args.sensors)?,
            be_verbose: cli_args.verbose,
            dry_run: cli_args.dry_run,
        };

        if app_config.be_verbose {
            dbg!(&app_config);
        }

        Ok(app_config)
    }
}
