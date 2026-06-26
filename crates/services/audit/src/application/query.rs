use std::sync::Arc;

use crate::application::dto::{ExportManifest, LedgerQuery};
use crate::application::port::{Clock, LedgerStore, WormArchive};
use crate::domain::{AuditRecord, CanonicalWriter, RecordHash};
use crate::error::AuditError;

/// The access-controlled read use case. Authorization (need-to-know + separation
/// of duties) and the recording of the read as its own `DATA_ACCESS` event happen
/// at the service boundary (Phase 5); this handler is the filtered ledger read
/// behind that gate.
pub struct QueryHandler {
    ledger: Arc<dyn LedgerStore>,
}

impl QueryHandler {
    pub fn new(ledger: Arc<dyn LedgerStore>) -> Self {
        Self { ledger }
    }

    pub async fn query(&self, spec: &LedgerQuery) -> Result<Vec<AuditRecord>, AuditError> {
        self.ledger.query(spec).await
    }
}

/// The formal export use case — produces a signed bundle for a subject/scope (DPO
/// / regulator / subject-access request) and returns a manifest referencing it in
/// object storage. The bytes never travel back through the application layer; the
/// caller resolves the `artifact_ref` out-of-band.
pub struct ExportHandler {
    ledger: Arc<dyn LedgerStore>,
    archive: Arc<dyn WormArchive>,
    clock: Arc<dyn Clock>,
}

impl ExportHandler {
    pub fn new(
        ledger: Arc<dyn LedgerStore>,
        archive: Arc<dyn WormArchive>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            ledger,
            archive,
            clock,
        }
    }

    pub async fn export(
        &self,
        export_id: &str,
        spec: &LedgerQuery,
    ) -> Result<ExportManifest, AuditError> {
        let records = self.ledger.query(spec).await?;
        let content = serialize_bundle(&records);
        let content_hash = RecordHash::digest(&content);
        let artifact_ref = self.archive.store_export(export_id, &content).await?;

        Ok(ExportManifest {
            export_id: export_id.to_owned(),
            record_count: records.len() as u64,
            content_hash,
            artifact_ref,
            generated_at: self.clock.now(),
        })
    }
}

/// Deterministic bundle bytes: each record's `(partition, sequence, record_hash)`
/// in order, length-prefixed. The content hash over this is the export's own
/// integrity check — a regulator can re-derive it.
fn serialize_bundle(records: &[AuditRecord]) -> Vec<u8> {
    let mut w = CanonicalWriter::new();
    w.u64(records.len() as u64);
    for r in records {
        w.str(r.partition().as_str())
            .u64(r.sequence())
            .str(r.record_hash().as_str());
    }
    w.as_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::Fixture;
    use crate::domain::event::fixtures;
    use crate::domain::{AuditEvent, EventCategory, SubjectPseudonym};

    fn event_for(id: &str, subject: &str) -> AuditEvent {
        let mut d = fixtures::draft(id, EventCategory::Moderation);
        d.subject = Some(SubjectPseudonym::new(subject).unwrap());
        AuditEvent::try_new(d).unwrap()
    }

    #[tokio::test]
    async fn query_filters_by_subject() {
        let fx = Fixture::new();
        fx.ingest().ingest(event_for("e1", "alice")).await.unwrap();
        fx.ingest().ingest(event_for("e2", "bob")).await.unwrap();

        let spec = LedgerQuery {
            subject: Some(SubjectPseudonym::new("alice").unwrap()),
            limit: 50,
            ..Default::default()
        };
        let rows = fx.query().query(&spec).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].event().subject().unwrap().as_str(), "alice");
    }

    #[tokio::test]
    async fn export_writes_a_bundle_and_returns_a_manifest() {
        let fx = Fixture::new();
        fx.ingest().ingest(event_for("e1", "alice")).await.unwrap();
        fx.ingest().ingest(event_for("e2", "alice")).await.unwrap();

        let spec = LedgerQuery {
            subject: Some(SubjectPseudonym::new("alice").unwrap()),
            limit: 50,
            ..Default::default()
        };
        let manifest = fx.export().export("exp-1", &spec).await.unwrap();

        assert_eq!(manifest.record_count, 2);
        assert_eq!(manifest.export_id, "exp-1");
        assert!(manifest.artifact_ref.contains("exp-1"));
        assert_eq!(fx.archive.export_count(), 1);
    }

    #[tokio::test]
    async fn export_content_hash_is_reproducible() {
        let fx = Fixture::new();
        fx.ingest().ingest(event_for("e1", "alice")).await.unwrap();
        let spec = LedgerQuery {
            subject: Some(SubjectPseudonym::new("alice").unwrap()),
            limit: 50,
            ..Default::default()
        };
        let m1 = fx.export().export("exp-1", &spec).await.unwrap();
        let m2 = fx.export().export("exp-2", &spec).await.unwrap();
        // Same records → same content hash, independent of export id.
        assert_eq!(m1.content_hash, m2.content_hash);
    }
}
