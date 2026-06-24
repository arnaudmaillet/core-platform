use opentelemetry::propagation::{Extractor, Injector};
use rdkafka::message::{BorrowedHeaders, Header, Headers, OwnedHeaders};

/// Mutable carrier that injects W3C TraceContext into a Kafka [`OwnedHeaders`] builder.
///
/// `OwnedHeaders::insert` uses a consuming builder pattern, so ownership is temporarily
/// moved out via `std::mem::replace` and then moved back after each insertion.
///
/// Obtain the final headers by consuming the injector:
/// ```rust,ignore
/// let mut injector = KafkaHeaderInjector::new();
/// inject_context(&mut injector);
/// let headers: OwnedHeaders = injector.into_headers();
/// ```
pub struct KafkaHeaderInjector(OwnedHeaders);

impl KafkaHeaderInjector {
    pub fn new() -> Self {
        Self(OwnedHeaders::new())
    }

    pub fn into_headers(self) -> OwnedHeaders {
        self.0
    }
}

impl Default for KafkaHeaderInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl Injector for KafkaHeaderInjector {
    fn set(&mut self, key: &str, value: String) {
        // OwnedHeaders::insert takes `self` and returns `Self`, so we swap
        // ownership in-place to satisfy the `&mut self` Injector contract.
        let headers = std::mem::replace(&mut self.0, OwnedHeaders::new());
        self.0 = headers.insert(Header {
            key,
            value: Some(value.as_str()),
        });
    }
}

/// Read-only carrier that extracts W3C TraceContext from incoming Kafka message headers.
///
/// The underlying [`BorrowedHeaders`] borrows from the rdkafka message and is valid for
/// the lifetime of the [`rdkafka::message::BorrowedMessage`] it came from.
pub struct KafkaHeaderExtractor<'a>(pub &'a BorrowedHeaders);

impl<'a> Extractor for KafkaHeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        (0..self.0.count()).find_map(|i| {
            let header = self.0.get(i);
            if header.key == key {
                header
                    .value
                    .and_then(|bytes| std::str::from_utf8(bytes).ok())
            } else {
                None
            }
        })
    }

    fn keys(&self) -> Vec<&str> {
        (0..self.0.count())
            .map(|i| self.0.get(i).key)
            .collect()
    }
}
