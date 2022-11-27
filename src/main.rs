//! Entry point of the application: listen to bluetooth advertisements
//! and call the sample handlers when appropriate.
use bluer::{Adapter, AdapterEvent, DeviceEvent, DeviceProperty};
use config_builder::AppConfig;
use futures::{pin_mut, stream::SelectAll, Stream, StreamExt};
use influxdb::Client;
use std::sync::Arc;
use tokio::sync::Mutex;

type InfluxDbProtectedConnector = Option<Arc<Mutex<Client>>>;

mod config_builder;
mod sample_handler;

/// Magic UUID value for advertised weather data, see the definition of the
/// [custom format](https://github.com/pvvx/ATC_MiThermometer/blob/master/README.md#custom-format-all-data-little-endian).
const WEATHER_SAMPLE_UUID_HEADER: u32 = 0x0000181a;

/// Setup the InfluxDb connector, wrapped in Arc and (tokio) Mutex, ready for subsequent usage.
fn setup_influx_connection(app_config: &AppConfig) -> InfluxDbProtectedConnector {
    match app_config.dry_run {
        true => None,
        false => {
            let influx_client = Client::new(&app_config.influx_conn, &app_config.influx_database);
            Some(Arc::new(Mutex::new(influx_client)))
        }
    }
}

/// Setup the bluetooth adapter, ready for subsequent usage.
async fn setup_bluetooth_adapter(
) -> Result<(impl Stream<Item = AdapterEvent>, Adapter), bluer::Error> {
    let session = bluer::Session::new().await?;
    let adapter = session.default_adapter().await?;
    println!(
        "Discovering devices using Bluetooth adapater {}",
        adapter.name()
    );
    adapter.set_powered(true).await?;

    let adapter_events = adapter.discover_devices().await?;
    Ok((adapter_events, adapter))
}

/// Handle an event linked to the bluetooth adapter, optionaly return a stream of device events.
async fn handle_adapter_evt<'a>(
    adapter_event: AdapterEvent,
    adapter: &Adapter,
    app_config: &'a AppConfig,
) -> Result<Option<impl Stream<Item = (DeviceEvent, &'a String)>>, bluer::Error> {
    let now = chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false);
    match adapter_event {
        AdapterEvent::DeviceAdded(addr) => {
            if let Some(room) = app_config.sensors_names.get(&addr) {
                println!("{now} Device {addr} found (room: {room})");
                let device = adapter.device(addr)?;
                let device_events = device.events().await?;
                let device_events = device_events.map(move |e| (e, room));
                Ok(Some(device_events))
            } else {
                println!("{now} Device {addr} found");
                Ok(None)
            }
        }
        AdapterEvent::DeviceRemoved(addr) => {
            match app_config.sensors_names.get(&addr) {
                Some(room) => {
                    println!("{now} Device {addr} removed (room: {room})")
                }
                None => println!("{now} Device {addr} removed"),
            };

            Ok(None)
        }
        AdapterEvent::PropertyChanged(_) => Ok(None),
    }
}

/// Handle a PropertyChanged event on a bluetooth device: filter the stream,
/// looking for data advertisement with the correct UUID.
async fn handle_dev_changed_prop_evt(
    changed_property: DeviceProperty,
    influx_client: &InfluxDbProtectedConnector,
    app_config: &AppConfig,
    room: &str,
) {
    if let DeviceProperty::ServiceData(service_data) = changed_property {
        for (uuid, raw_sample) in service_data {
            if uuid.as_fields().0 == WEATHER_SAMPLE_UUID_HEADER {
                sample_handler::handle_sample(
                    raw_sample,
                    room,
                    influx_client,
                    &app_config.influx_measurement,
                    app_config.be_verbose,
                )
                .await;
            }
        }
    }
}

/// Run the whole application.
#[tokio::main(flavor = "current_thread")]
async fn main() -> bluer::Result<()> {
    let app_config = AppConfig::get_from_cli_inputs().unwrap();

    let influx_client = setup_influx_connection(&app_config);

    let mut device_events = SelectAll::new();
    let (adapter_events, adapter) = setup_bluetooth_adapter().await?;
    pin_mut!(adapter_events);

    loop {
        tokio::select! {
            Some(adapter_evt) = adapter_events.next() => {
                // Handle some new event related to the bluetooth adapter
                if let Some(new_dev_evts) = handle_adapter_evt(adapter_evt, &adapter, &app_config).await?{
                    // Push the device events stream to tokio
                    device_events.push(new_dev_evts);
                };
            },
            Some((DeviceEvent::PropertyChanged(prop), room)) = device_events.next() => {
                // Handle a new event related to a linked device
                handle_dev_changed_prop_evt(prop, &influx_client, &app_config, room).await;
            },
            else => break
        }
    }
    Ok(())
}
