//! MQTT Appender for log4rs.
//! 
//! This will format log messages, remove trailing newlines, and publish the message to an MQTT topic.
//! You can programatically create one or use a log4rs.yml definition. In order to use the YAML
//! you'll need to register the `MqttAppenderDeserializer` with `register`.
//! 
//! # Examples
//! 
//! ## Create an MQTT logger programatically
//! ```
//! let mqtt_log = MqttAppender::builder()
//!     .topic("logs")
//!     .client_id("log_client")
//!     .build();
//! let log_config = Config::builder()
//!     .appender(Appender::builder().build("mqtt", Box::new(mqtt_log)))
//!     .build(Root::builder().appender("mqtt").build(LevelFilter::Info))
//!     .unwrap();
//! log4rs::init_config(log_config).unwrap();
//! ```
//! 
//! ## Create an MQTT logger with YAML
//! ```yaml
//! appenders:
//!   mqtt:
//!     kind: mqtt
//!     mqtt_server: mqtt://mosquitto.local:1883
//!     mqtt_client_id: app_logger
//! root:
//!   level: info
//!   appenders:
//!     - mqtt
//! ```
//! 
//! # Warning
//! Ensure that `paho_mqtt_c` and `paho_mqtt` log targets do not log with this appender, especially when logging DEBUG.
//! You'll get a recursive call to the MQTT manager setup and the RwLock will level release. If you see sudden
//! application hangs, try decreasing the level or removing the MQTT logger and see if that fixes the problem.

extern crate async_std;
extern crate derivative;
extern crate log;
extern crate log4rs;
extern crate paho_mqtt;

use std::{io::BufWriter, time::Duration};
use async_std::task::block_on;
use derivative::Derivative;
use log::Record;
use log4rs::{encode::{EncoderConfig, Encode, pattern::PatternEncoder, self}, append::Append, config::{Deserialize, Deserializers}};
use paho_mqtt as mqtt;

#[derive(Clone, Eq, PartialEq, Hash, Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
/// Configuration structure for the MQTT appender
pub struct MqttAppenderConfig {
    topic: Option<String>,
    qos: Option<i32>,
    encoder: Option<EncoderConfig>,
    mqtt_server: Option<String>,
    mqtt_client_id: Option<String>,
}

#[derive(Derivative)]
#[derivative(Debug)]
/// Main MQTT appender structure
pub struct MqttAppender {
    topic: String,
    qos: i32,
    encoder: Box<dyn Encode>,
    #[derivative(Debug="ignore")]
    mqtt: mqtt::AsyncClient,
}

impl Append for MqttAppender {
    /// Append to the MQTT stream.
    /// 
    /// This encodes the [`Record`] in a string buffer,
    /// strips the trailing newline, then sends it to the MQTT topic.
    fn append(&self, record: &Record) -> anyhow::Result<()> {
        let mut buffer = StrBuilder { buf: BufWriter::new(Vec::new()) };
        self.encoder.encode(&mut buffer, record)?;
        let payload = String::from_utf8_lossy(buffer.buf.buffer()).to_string();
        let message = mqtt::MessageBuilder::new()
            .topic(self.topic.as_str())
            .qos(self.qos)
            .payload(payload.strip_suffix("\n").unwrap())
            .finalize();
        block_on(self.mqtt.publish(message))?;
        Ok(())
    }

    /// Do nothing
    fn flush(&self) {}
}

impl MqttAppender {
    /// Create a new builder for MqttAppender.
    pub fn builder() -> MqttAppenderBuilder {
        MqttAppenderBuilder {
            topic: None,
            qos: None,
            encoder: None,
            mqtt_server: None,
            mqtt_client_id: None,
        }
    }
}

/// Configuration builder.
pub struct MqttAppenderBuilder {
    topic: Option<String>,
    qos: Option<i32>,
    encoder: Option<Box<dyn Encode>>,
    mqtt_server: Option<String>,
    mqtt_client_id: Option<String>,
}

impl MqttAppenderBuilder {
    /// Sets the output encoder for the `MqttAppender`.
    pub fn encoder(mut self, encoder: Box<dyn Encode>) -> MqttAppenderBuilder {
        self.encoder = Some(encoder);
        self
    }

    /// Sets the MQTT topic to send logs to.
    /// Defaults to "logging"
    pub fn topic(mut self, topic: &str) -> MqttAppenderBuilder {
        self.topic = Some(topic.to_string());
        self
    }

    /// Sets the MQTT QOS to use when sending logs.
    /// Defaults to 0.
    pub fn qos(mut self, qos: i32) -> MqttAppenderBuilder {
        self.qos = Some(qos);
        self
    }

    /// Sets the MQTT server URI.
    /// Defaults to mqtt://localhost:1883
    pub fn mqtt_server(mut self, host: &str) -> MqttAppenderBuilder {
        self.mqtt_server = Some(host.to_string());
        self
    }

    /// Sets the MQTT client name.
    /// Defaults to a randomly generated name.
    pub fn mqtt_client_id(mut self, client_id: &str) -> MqttAppenderBuilder {
        self.mqtt_client_id = Some(client_id.to_string());
        self
    }

    /// Consumes the `MqttAppenderBuilder`, producing an `MqttAppender`.
    pub fn build(self) -> MqttAppender {
        let mut copts = mqtt::CreateOptionsBuilder::new()
            .server_uri(self.mqtt_server.unwrap_or_else(|| "mqtt://localhost:1883".to_string()));
        if let Some(client_id) = self.mqtt_client_id {
            copts = copts.client_id(client_id);
        }
        let mqtt_client = mqtt::AsyncClient::new(copts.finalize()).expect("Unable to create MQTT client");
        let opts = mqtt::ConnectOptionsBuilder::new()
            .connect_timeout(Duration::from_secs(5))
            .automatic_reconnect(Duration::from_secs(5), Duration::from_secs(300))
            .finalize();
        block_on(mqtt_client.connect(opts)).unwrap();

        MqttAppender {
            topic: self.topic.unwrap_or_else(|| "logging".to_string()),
            qos: self.qos.unwrap_or_else(|| 0),
            encoder: self.encoder.unwrap_or_else(|| Box::new(PatternEncoder::default())),
            mqtt: mqtt_client,
        }
    }
}

/// A deserializer for the `MqttAppender`.
/// 
/// # Configuration
/// 
/// ```yaml
/// kind: mqtt
/// 
/// # The topic used to publish logs. Defaults to `logging`
/// topic: log_messages
/// 
/// # The QOS value to use for MQTT publishing. Must be a valid QOS (0, 1, 2) and defaults to 0.
/// qos: 1
/// 
/// # The encoder to use to format output. Defaults to `kind: pattern`.
/// encoder:
///   kind: pattern
/// 
/// # The MQTT server URI. If not specified, defaults to mqtt://localhost:1883
/// mqtt_server: mqtt://localhost:1883
/// 
/// # The MQTT client ID. If not speficied, use the paho default.
/// mqtt_client_id: app_logger
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct MqttAppenderDeserializer;

impl Deserialize for MqttAppenderDeserializer {
    type Trait = dyn Append;

    type Config = MqttAppenderConfig;

    fn deserialize(&self, config: MqttAppenderConfig, deserializers: &Deserializers) -> anyhow::Result<Box<Self::Trait>> {
        let mut appender = MqttAppender::builder();
        if let Some(topic) = config.topic {
            appender = appender.topic(topic.as_str());
        }
        if let Some(qos) = config.qos {
            appender = appender.qos(qos);
        }
        if let Some(encoder) = config.encoder {
            appender = appender.encoder(deserializers.deserialize(&encoder.kind, encoder.config)?);
        }
        if let Some(mqtt_server) = config.mqtt_server {
            appender = appender.mqtt_server(mqtt_server.as_str());
        }
        if let Some(mqtt_client_id) = config.mqtt_client_id {
            appender = appender.mqtt_client_id(&mqtt_client_id.as_str());
        }
        Ok(Box::new(appender.build()))
    }
}

/// Register deserializer for creating MQTT appender based on log4rs configuration file.
pub fn register(deserializers: &mut log4rs::config::Deserializers) {
    deserializers.insert("mqtt", MqttAppenderDeserializer);
    deserializers.insert("mqtt", MqttAppenderDeserializer);
}

/// We need to do this to avoid E0117 since BufWriter and encode::Write are both external
/// Le sigh.
struct StrBuilder { buf: BufWriter<Vec<u8>> }

impl std::io::Write for StrBuilder {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buf.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.buf.flush()
    }
}
impl encode::Write for StrBuilder {}
