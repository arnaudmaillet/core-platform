//! Print the generated topic-wiring block for docs/domain/EVENT_CATALOG.md, rendered from the
//! event-topology registry. Stdout only — `tools/event-catalog/sync.sh` splices it into the doc
//! between the BEGIN/END markers.
//!
//!   cargo run -p event-topology --bin gen-event-catalog

fn main() {
    println!("{}", event_topology::render_catalog_block());
}
