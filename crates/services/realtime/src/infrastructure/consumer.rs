//! Inbound runtime glue for the dispatcher's Kafka consumers.
//!
//! Lets the shared at-least-once
//! [`run_consumer`](transport::kafka::consumer::run_consumer) runner classify a
//! [`RealtimeError`]: delegate to the error's own retry verdict, so transient
//! fabric faults (`RTM-4001` / `RTM-4002`) retry while decode/domain faults
//! (`RTM-8001`, `RTM-9xxx`) are poison and dead-letter immediately. An offline
//! recipient is never an error (the fan-out handler folds it into `Ok`), so it
//! commits the offset normally.
//!
//! The concrete consumer wiring (decode each record → `FanOutHandler::fan_out`
//! under `run_consumer`) is assembled in the dispatcher composition root (Phase 5).

use crate::error::RealtimeError;

impl transport::kafka::consumer::ClassifyError for RealtimeError {
    fn is_retryable(&self) -> bool {
        <Self as error::AppError>::is_retryable(self)
    }
}
