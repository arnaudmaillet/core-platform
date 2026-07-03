/// Shared Kafka client settings inherited by both producers and consumers.
#[derive(Debug, Clone)]
pub struct KafkaClientConfig {
    /// Comma-separated list of broker addresses, e.g. `kafka-0:9092,kafka-1:9092`.
    pub brokers: String,

    /// Security protocol. Use `"PLAINTEXT"` for in-cluster without TLS (default),
    /// `"SASL_SSL"` for cloud-managed brokers (Confluent Cloud, MSK, etc.).
    pub security_protocol: String,

    /// SASL mechanism, e.g. `"PLAIN"`, `"SCRAM-SHA-256"`. Required when
    /// `security_protocol` is `SASL_PLAINTEXT` or `SASL_SSL`.
    pub sasl_mechanism: Option<String>,

    /// SASL username.
    pub sasl_username: Option<String>,

    /// SASL password.
    pub sasl_password: Option<String>,

    /// Log-level forwarded to rdkafka's internal debug logger.
    /// Useful values: `"all"`, `"consumer"`, `"producer"`, `"topic"`.
    pub rdkafka_debug: Option<String>,

    /// CA bundle path for TLS broker connections (`ssl.ca.location`).
    ///
    /// REQUIRED KNOWLEDGE for this workspace: librdkafka links a VENDORED,
    /// statically-built OpenSSL (`rdkafka/ssl-vendored`), which has no baked-in
    /// system CA path — without an explicit location every TLS handshake fails
    /// certificate verification and surfaces only as a generic timeout (found
    /// live on the staging bring-up: create_topics OperationTimedOut while a
    /// dynamically-linked kcat from the same pod network succeeded). Defaults
    /// to the Debian bundle the runtime image installs
    /// (`/etc/ssl/certs/ca-certificates.crt`); override with
    /// `KAFKA_SSL_CA_LOCATION` (e.g. macOS local dev against a cloud broker).
    /// Only applied when `security_protocol` uses SSL.
    pub ssl_ca_location: Option<String>,
}

/// Debian/Ubuntu CA bundle path — what `deploy/Dockerfile`'s runtime stage
/// provides via `ca-certificates`.
const DEFAULT_SSL_CA_LOCATION: &str = "/etc/ssl/certs/ca-certificates.crt";

impl Default for KafkaClientConfig {
    fn default() -> Self {
        Self {
            brokers: "localhost:9092".to_string(),
            security_protocol: "PLAINTEXT".to_string(),
            sasl_mechanism: None,
            sasl_username: None,
            sasl_password: None,
            rdkafka_debug: None,
            ssl_ca_location: None,
        }
    }
}

impl KafkaClientConfig {
    pub fn new(brokers: impl Into<String>) -> Self {
        Self {
            brokers: brokers.into(),
            ..Default::default()
        }
    }

    /// Populates settings from environment variables using the standard
    /// `KAFKA_BROKERS`, `KAFKA_SECURITY_PROTOCOL`, `KAFKA_SASL_*` naming.
    pub fn from_env() -> Self {
        Self {
            brokers: std::env::var("KAFKA_BROKERS")
                .unwrap_or_else(|_| "localhost:9092".to_string()),
            security_protocol: std::env::var("KAFKA_SECURITY_PROTOCOL")
                .unwrap_or_else(|_| "PLAINTEXT".to_string()),
            sasl_mechanism: std::env::var("KAFKA_SASL_MECHANISM").ok(),
            sasl_username: std::env::var("KAFKA_SASL_USERNAME").ok(),
            sasl_password: std::env::var("KAFKA_SASL_PASSWORD").ok(),
            rdkafka_debug: std::env::var("KAFKA_DEBUG").ok(),
            ssl_ca_location: std::env::var("KAFKA_SSL_CA_LOCATION").ok(),
        }
    }

    /// Materialises these settings into an [`rdkafka::ClientConfig`].
    ///
    /// Public (not `pub(crate)`) so admin tooling outside this crate — the
    /// topic-provisioner — connects with the exact same broker/SASL settings
    /// the fleet's producers and consumers use.
    pub fn to_rdkafka(&self) -> rdkafka::config::ClientConfig {
        let mut cfg = rdkafka::config::ClientConfig::new();
        cfg.set("bootstrap.servers", &self.brokers)
            .set("security.protocol", &self.security_protocol);

        if let Some(m) = &self.sasl_mechanism {
            cfg.set("sasl.mechanism", m);
        }
        if let Some(u) = &self.sasl_username {
            cfg.set("sasl.username", u);
        }
        if let Some(p) = &self.sasl_password {
            cfg.set("sasl.password", p);
        }
        if let Some(d) = &self.rdkafka_debug {
            cfg.set("debug", d);
        }

        // Vendored OpenSSL has no system CA path — point it at the runtime
        // image's bundle whenever the connection uses TLS (see field docs).
        if self.security_protocol.to_ascii_uppercase().contains("SSL") {
            cfg.set(
                "ssl.ca.location",
                self.ssl_ca_location.as_deref().unwrap_or(DEFAULT_SSL_CA_LOCATION),
            );
        }

        cfg
    }
}
