//! Anti-malware scanning. The real scanner is an external sidecar (ClamAV-style);
//! this ships a log stub that passes everything, mirroring `moderation`'s
//! `LogClassifierGateway`. A real adapter slots in behind the same port.

pub mod log_malware_scanner;

pub use log_malware_scanner::LogMalwareScanner;
