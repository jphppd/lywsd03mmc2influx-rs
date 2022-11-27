use bluer::Address;
use chrono::{DateTime, Utc};
use influxdb::InfluxDbWriteable;
use std::convert::TryInto;
use std::error::Error;

/// Battery data.
#[derive(Debug)]
struct Battery {
    voltage: f32,
    level: u8,
}

/// Deserialized data from the sensors with additional metadata.
#[derive(Debug)]
struct Sample<'a> {
    timestamp: DateTime<Utc>,
    sensor_addr: Address,
    room: &'a str,
    temperature: f32,
    humidity: f32,
    battery: Battery,
}

/// InfluxDB structure for a new point (single data record).
#[derive(Debug, InfluxDbWriteable)]
struct InfluxPoint<'a> {
    time: DateTime<Utc>,
    #[influxdb(tag)]
    sensor: String,
    #[influxdb(tag)]
    room: &'a str,
    temperature: f32,
    humidity: f32,
    battery_voltage: f32,
    battery_level: i32,
}

impl<'a> TryFrom<(Vec<u8>, &'a str)> for Sample<'a> {
    type Error = Box<dyn Error>;

    fn try_from((value, room): (Vec<u8>, &'a str)) -> Result<Self, Self::Error> {
        let arr: &[u8; 15] = value[..].try_into()?;

        let mut sensor_addr = [0_u8; 6];
        sensor_addr.copy_from_slice(&arr[0..6]);
        sensor_addr.reverse();
        let sensor_addr = Address::new(sensor_addr);

        let temperature = (i16::from_le_bytes(arr[6..8].try_into()?) as f32) / 100.;
        let humidity = (u16::from_le_bytes(arr[8..10].try_into()?) as f32) / 100.;
        let battery = Battery {
            voltage: (u16::from_le_bytes(arr[10..12].try_into()?) as f32) / 1000.,
            level: arr[12],
        };

        Ok(Sample {
            timestamp: Utc::now(),
            sensor_addr,
            temperature,
            humidity,
            battery,
            room,
        })
    }
}

impl<'a> From<&Sample<'a>> for InfluxPoint<'a> {
    fn from(measurement: &Sample<'a>) -> Self {
        Self {
            time: measurement.timestamp,
            sensor: measurement.sensor_addr.to_string(),
            room: measurement.room,
            temperature: measurement.temperature,
            humidity: measurement.humidity,
            battery_voltage: measurement.battery.voltage,
            battery_level: measurement.battery.level.into(),
        }
    }
}

/// Handle one unique weather sample.
pub async fn handle_sample<'a>(
    raw_sample: Vec<u8>,
    room: &str,
    influx_client: &crate::InfluxDbProtectedConnector,
    influx_measurement: &str,
    be_verbose: bool,
) {
    // (Try to) deserialize the data
    let sample = Sample::try_from((raw_sample, room)).unwrap();

    if be_verbose {
        dbg!(&sample);
    }

    // Run the real query only is the client is "Some" (that is, no dry-run)
    if let Some(influx_client) = influx_client {
        let influx_client = influx_client.clone();
        let influx_client = influx_client.lock().await;
        let point = InfluxPoint::from(&sample);
        let query = point.into_query(influx_measurement);
        influx_client.query(query).await.unwrap();
    }
}
