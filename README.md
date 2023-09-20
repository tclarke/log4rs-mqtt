[![crates.io](https://img.shields.io/crates/v/log4rs-mqtt.svg?maxAge=3600)](https://crates.io/crates/log4rs-mqtt)
![MIT/Apache 2.0](https://img.shields.io/badge/license-MIT%2FApache_2.0-blue.svg)
# log4rs-mqtt

`log4rs-mqtt` - MQTT appender for the log4rs based on [PAHO MQTT](https://github.com/eclipse/paho.mqtt.rust).

[Documentation on docs.rs](https://docs.rs/crate/log4rs-mqtt)

Features:
* Specify the MQTT server to use.
* Specify the MQTT topic used for logging.

## Usage

Add this to your Cargo.toml:

```toml
[dependencies]
log4rs-mqtt = "1.0"
```

### Initialization based on configuration file

Example configuration file:

```yaml
appenders:
  mqtt:
    kind: mqtt
    mqtt_server: mqtt://mosquito.cluster.local:1883
    mqtt_client_id: log_client
    topic: logs
    qos: 1
    encoder:
      pattern: "{M} - {m}"
root:
  level: info
  appenders:
    - mqtt
```

Example code:

```rust,no_run
#[macro_use]
extern crate log;
extern crate log4rs;
extern crate log4rs_mqtt;

fn main() {
    let mut deserializers = log4rs::file::Deserializers::new();
    log4rs_mqtt::register(&mut deserializers);

    // Note that configuration file should have right extension, otherwise log4rs will fail to
    // recognize format.
    log4rs::init_file("test.yaml", deserializers).unwrap();

    info!("Example information message");
    warn!("Example warning message");
    error!("Example error message");
}
```

### Manual initialization

Example code:

```rust,no_run
#[macro_use]
extern crate log;
extern crate log4rs;
extern crate log4rs_mqtt;

fn main() {
    // Use custom PatternEncoder to avoid duplicate timestamps in logs.
    let encoder = Box::new(log4rs::encode::pattern::PatternEncoder::new("{M} - {m}"));

    let appender = Box::new(
        log4rs_mqtt::MqttAppender::builder()
            .encoder(encoder)
            .mqtt_server("mqtt://mosquitto.cluster.local:1883")
            .mqtt_client_id("log_client")
            .qos(1)
            .topic("logs")
            .build(),
    );

    let config = log4rs::config::Config::builder()
        .appender(log4rs::config::Appender::builder().build(
            "mqtt",
            appender,
        ))
        .build(log4rs::config::Root::builder().appender("mqtt").build(
            log::LevelFilter::Info,
        ))
        .unwrap();
    log4rs::init_config(config).unwrap();

    info!("Example information message");
    warn!("Example warning message");
    error!("Example error message");
}
```
