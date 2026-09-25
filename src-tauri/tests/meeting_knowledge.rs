#[path = "../src/meeting_knowledge.rs"]
mod meeting_knowledge;

use chrono::NaiveDate;
use meeting_knowledge::{
    actual_vs_budget_percent, chunk_document, compute_selected_actual_vs_budget,
    conflicting_metric_sources, open_private_payload, previous_year_period, seal_private_payload,
    search_selected_knowledge, CalendarBasis, ChunkLocator, CitationLocator, Decimal,
    EncryptedKnowledgePayload, EvidenceCitation, KnowledgeKeyRef, KnowledgeQuery,
    KnowledgeReadBoundary, KnowledgeReadGrant, KnowledgeScopeKind, KnowledgeSearchStatus,
    MetricArithmeticError, MetricBasis, MetricObservation, ParsedDocument, ParsedPage,
    PrivateAssetContext, PrivateAssetPurpose, ReviewedPeopleLinkRef, StoredKnowledgeChunk,
};
use std::collections::BTreeSet;
use std::sync::Mutex;
use uuid::Uuid;
use zeroize::Zeroizing;

const VAULT_ID: &str = "vault-fixture-1";
const COLLECTION_ID: &str = "selected-references";

fn key_ref() -> KnowledgeKeyRef {
    KnowledgeKeyRef::new(format!("knowledge:{}", Uuid::from_u128(7))).unwrap()
}

fn private_context(chunk_id: &str) -> PrivateAssetContext {
    PrivateAssetContext {
        vault_id: VAULT_ID.to_string(),
        entity_id: chunk_id.to_string(),
        version: 1,
        purpose: PrivateAssetPurpose::KnowledgeDocument,
    }
}

fn citation(document_version_id: &str, digest: &str) -> EvidenceCitation {
    EvidenceCitation {
        collection_id: COLLECTION_ID.to_string(),
        document_id: "document-1".to_string(),
        document_version_id: document_version_id.to_string(),
        document_version_number: 1,
        source_version: "v1".to_string(),
        content_sha256: digest.to_string(),
        locator: CitationLocator::TextSpan {
            start_line: 1,
            end_line: 2,
            start_char: 0,
            end_char: 32,
        },
        retrieved_at: "2026-09-24T00:00:00Z".to_string(),
        read_grant_id: "grant-1".to_string(),
        acl_revision: 3,
    }
}

fn encrypted_chunk(collection: &str, id: &str, document: &str, text: &str) -> StoredKnowledgeChunk {
    let context = private_context(id);
    let mut key = [0_u8; 32];
    key[0] = 42;
    let payload = seal_private_payload(&key, key_ref(), &context, text.as_bytes()).unwrap();
    StoredKnowledgeChunk {
        id: id.to_string(),
        collection_id: collection.to_string(),
        document_id: document.to_string(),
        document_version_id: format!("{document}-version-1"),
        document_version_number: 1,
        source_version: "v1".to_string(),
        content_sha256: "a".repeat(64),
        acl_revision: 3,
        key_ref: key_ref(),
        private_context: context,
        locator: CitationLocator::TextSpan {
            start_line: 8,
            end_line: 9,
            start_char: 120,
            end_char: 120 + text.chars().count() as u64,
        },
        payload,
    }
}

struct FixtureBoundary {
    key: [u8; 32],
    allowed: BTreeSet<String>,
    current: bool,
    opened: Mutex<Vec<String>>,
}

impl FixtureBoundary {
    fn new(allowed: &[&str], current: bool) -> Self {
        let mut key = [0_u8; 32];
        key[0] = 42;
        Self {
            key,
            allowed: allowed.iter().map(|item| (*item).to_string()).collect(),
            current,
            opened: Mutex::new(Vec::new()),
        }
    }
}

impl KnowledgeReadBoundary for FixtureBoundary {
    fn authorize(
        &self,
        _query: &KnowledgeQuery,
        selected_collections: &BTreeSet<String>,
        chunk: &StoredKnowledgeChunk,
    ) -> Result<Option<KnowledgeReadGrant>, String> {
        if !selected_collections.contains(&chunk.collection_id)
            || !self.allowed.contains(&chunk.collection_id)
        {
            return Ok(None);
        }
        Ok(Some(KnowledgeReadGrant {
            grant_id: "read-grant-fixture".to_string(),
            owner_principal_ref: "owner-fixture".to_string(),
            vault_id: VAULT_ID.to_string(),
            account_generation: 2,
            identity_generation: 4,
            private_asset_id: chunk.id.clone(),
            collection_id: chunk.collection_id.clone(),
            document_id: chunk.document_id.clone(),
            document_version_id: chunk.document_version_id.clone(),
            acl_revision: chunk.acl_revision,
        }))
    }

    fn open(
        &self,
        grant: &KnowledgeReadGrant,
        context: &PrivateAssetContext,
        _key_ref: &KnowledgeKeyRef,
        payload: &EncryptedKnowledgePayload,
    ) -> Result<Zeroizing<Vec<u8>>, String> {
        self.opened.lock().unwrap().push(grant.document_id.clone());
        open_private_payload(&self.key, context, payload)
    }

    fn revalidate(&self, _grant: &KnowledgeReadGrant) -> Result<bool, String> {
        Ok(self.current)
    }
}

fn query(collections: Vec<String>) -> KnowledgeQuery {
    KnowledgeQuery::local("session-1".to_string(), "ยอดขาย".to_string(), collections)
}

#[test]
fn private_payload_is_bound_to_vault_asset_and_version() {
    let key = [9_u8; 32];
    let context = private_context("chunk-1");
    let sealed = seal_private_payload(&key, key_ref(), &context, "ยอดขาย".as_bytes()).unwrap();
    let opened = open_private_payload(&key, &context, &sealed).unwrap();
    assert_eq!(opened.as_slice(), "ยอดขาย".as_bytes());

    let mut wrong_context = context.clone();
    wrong_context.vault_id = "another-vault".to_string();
    assert!(open_private_payload(&key, &wrong_context, &sealed).is_err());

    let mut corrupt = sealed;
    corrupt.ciphertext[0] ^= 1;
    assert!(open_private_payload(&key, &context, &corrupt).is_err());
}

#[test]
fn selected_scope_filters_before_decryption_and_preserves_exact_citation() {
    let allowed = encrypted_chunk(
        COLLECTION_ID,
        "chunk-allowed",
        "document-allowed",
        "รายงานยอดขายสุทธิปี 2025",
    );
    let foreign = encrypted_chunk(
        "another-collection",
        "chunk-foreign",
        "document-foreign",
        "ยอดขายลับจากอีกคลัง",
    );
    let boundary = FixtureBoundary::new(&[COLLECTION_ID], true);
    let response = search_selected_knowledge(
        &boundary,
        &[foreign, allowed],
        &query(vec![COLLECTION_ID.to_string()]),
    )
    .unwrap();
    assert_eq!(response.status, KnowledgeSearchStatus::Found);
    assert_eq!(response.scope, KnowledgeScopeKind::SelectedCollections);
    assert_eq!(response.evidence.len(), 1);
    assert_eq!(
        response.evidence[0].citation.document_id,
        "document-allowed"
    );
    assert_eq!(
        response.evidence[0].citation.document_version_id,
        "document-allowed-version-1"
    );
    assert_eq!(
        response.evidence[0].citation.read_grant_id,
        "read-grant-fixture"
    );
    assert_eq!(
        boundary.opened.lock().unwrap().as_slice(),
        &["document-allowed"]
    );
}

#[test]
fn source_revocation_during_retrieval_returns_no_excerpt() {
    let chunk = encrypted_chunk(COLLECTION_ID, "chunk-1", "document-1", "รายงานยอดขายสุทธิ");
    let boundary = FixtureBoundary::new(&[COLLECTION_ID], false);
    assert!(search_selected_knowledge(
        &boundary,
        &[chunk],
        &query(vec![COLLECTION_ID.to_string()]),
    )
    .is_err());
}

#[test]
fn imported_text_chunks_keep_character_and_line_provenance() {
    let text = "หัวข้อ\nยอดขาย 42".to_string();
    let document = ParsedDocument {
        parser_version: "fung-knowledge-extractor/0.1.0".to_string(),
        parser_fingerprint: "b".repeat(64),
        dependency: meeting_knowledge::ParserDependency {
            name: "python-stdlib".to_string(),
            version: "stdlib".to_string(),
            license: "Python-PSF-2.0".to_string(),
        },
        content_sha256: "a".repeat(64),
        mime_type: "text/plain".to_string(),
        source_bytes: text.len() as u64,
        pages: vec![ParsedPage {
            page_index: None,
            start_char: 0,
            end_char: text.chars().count() as u64,
            text,
        }],
        warnings: vec![],
    };
    let chunks = chunk_document(&document);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].locator.start_line, Some(1));
    assert_eq!(chunks[0].locator.end_line, Some(2));
    assert_eq!(chunks[0].locator.start_char, 0);
    assert_eq!(
        chunks[0].locator.end_char,
        document.pages[0].text.chars().count() as u64
    );
    assert!(matches!(
        chunks[0].locator.as_citation_locator().unwrap(),
        CitationLocator::TextSpan {
            start_line: 1,
            end_line: 2,
            ..
        }
    ));

    let page_locator = ChunkLocator {
        page_index: Some(0),
        start_char: 3,
        end_char: 12,
        start_line: None,
        end_line: None,
    };
    assert!(matches!(
        page_locator.as_citation_locator().unwrap(),
        CitationLocator::PdfPage {
            page_number: 1,
            start_char: 3,
            end_char: 12
        }
    ));
}

fn observation(basis: MetricBasis, value: &str, version: &str) -> MetricObservation {
    MetricObservation {
        metric_key: "net_sales".to_string(),
        organization_ref: "company-a".to_string(),
        period_start: "2025-01-01".to_string(),
        period_end: "2025-12-31".to_string(),
        calendar: "gregorian".to_string(),
        unit: "currency".to_string(),
        currency: Some("THB".to_string()),
        scale: "unit".to_string(),
        basis,
        value: Decimal::parse(value).unwrap(),
        citation: citation(
            version,
            &(version.chars().next().unwrap_or('a').to_string().repeat(64)),
        ),
    }
}

#[test]
fn metric_math_is_exact_and_keeps_citation_lineage() {
    let actual = observation(MetricBasis::Actual, "110.00", "actual-v1");
    let budget = observation(MetricBasis::Budget, "100.00", "budget-v1");
    let result = compute_selected_actual_vs_budget(&[actual], &[budget]).unwrap();
    assert_eq!(result.percentage.format(), "10");
    assert_eq!(result.citations.len(), 2);

    let actual = observation(MetricBasis::Actual, "1", "actual-third");
    let budget = observation(MetricBasis::Budget, "3", "budget-third");
    assert_eq!(
        actual_vs_budget_percent(&actual, &budget).unwrap().format(),
        "-66.67"
    );
}

#[test]
fn metric_value_is_encrypted_and_bound_to_its_vault_and_observation() {
    let key = [29_u8; 32];
    let key_ref = key_ref();
    let value = Decimal::parse("42424242.125").unwrap();
    let encoded = meeting_knowledge::seal_metric_observation_value(
        &key,
        &key_ref,
        VAULT_ID,
        "metric-observation-a",
        value,
    )
    .unwrap();
    let payload: EncryptedKnowledgePayload = serde_json::from_str(&encoded).unwrap();
    let clear_value = value.format();
    assert_ne!(payload.ciphertext.as_slice(), clear_value.as_bytes());
    assert!(
        !encoded.contains("42424242.125"),
        "the persisted envelope must not contain the decimal canary"
    );
    assert_eq!(
        meeting_knowledge::open_metric_observation_value(
            &key,
            &key_ref,
            VAULT_ID,
            "metric-observation-a",
            &encoded,
        )
        .unwrap(),
        value,
    );
    assert!(meeting_knowledge::open_metric_observation_value(
        &key,
        &key_ref,
        VAULT_ID,
        "metric-observation-b",
        &encoded,
    )
    .is_err());
    assert!(meeting_knowledge::open_metric_observation_value(
        &key,
        &key_ref,
        "another-vault",
        "metric-observation-a",
        &encoded,
    )
    .is_err());

    let trailing_zero_value = Decimal::parse("12.3400").unwrap();
    let trailing_zero_payload = meeting_knowledge::seal_metric_observation_value(
        &key,
        &key_ref,
        VAULT_ID,
        "metric-observation-trailing-zero",
        trailing_zero_value,
    )
    .unwrap();
    assert_eq!(
        meeting_knowledge::open_metric_observation_value(
            &key,
            &key_ref,
            VAULT_ID,
            "metric-observation-trailing-zero",
            &trailing_zero_payload,
        )
        .unwrap(),
        trailing_zero_value
    );
}

#[test]
fn metric_zero_conflict_and_dimension_mismatch_fail_closed() {
    let actual = observation(MetricBasis::Actual, "10", "actual-v1");
    let zero_budget = observation(MetricBasis::Budget, "0", "budget-v1");
    assert_eq!(
        actual_vs_budget_percent(&actual, &zero_budget),
        Err(MetricArithmeticError::NotComputable)
    );

    let mut other_period = observation(MetricBasis::Budget, "10", "budget-v2");
    other_period.period_start = "2024-01-01".to_string();
    other_period.period_end = "2024-12-31".to_string();
    assert_eq!(
        actual_vs_budget_percent(&actual, &other_period),
        Err(MetricArithmeticError::IncompatibleObservations)
    );

    let first = observation(MetricBasis::Actual, "10", "actual-v1");
    let second = observation(MetricBasis::Actual, "11", "actual-v2");
    let conflicting = conflicting_metric_sources(&[first.clone(), second.clone()]);
    assert_eq!(
        conflicting,
        BTreeSet::from(["actual-v1".to_string(), "actual-v2".to_string()])
    );
    assert_eq!(
        compute_selected_actual_vs_budget(
            &[first, second],
            &[observation(MetricBasis::Budget, "10", "budget-v1")],
        ),
        Err(MetricArithmeticError::ConflictingSources)
    );
}

#[test]
fn previous_year_uses_explicit_calendar_and_fiscal_unknown_clarifies() {
    let date = NaiveDate::from_ymd_opt(2026, 9, 21).unwrap();
    let previous = previous_year_period(date, CalendarBasis::Gregorian);
    assert!(matches!(
        previous,
        meeting_knowledge::TemporalResolution::Resolved { ref period }
            if period.start == "2025-01-01" && period.end == "2025-12-31" && period.displayed_year == 2025
    ));
    assert!(matches!(
        previous_year_period(date, CalendarBasis::FiscalNeedsConfiguration),
        meeting_knowledge::TemporalResolution::NeedsClarification { .. }
    ));
}

#[test]
fn people_link_requires_review_revision_and_never_uses_display_name() {
    let link = ReviewedPeopleLinkRef {
        identity_link_id: "identity-link:abc".to_string(),
        expected_revision: 4,
        evidence_revision: 4,
        person_ref: "person-ref-1".to_string(),
        state: "confirmed".to_string(),
    };
    assert!(link.validate().is_ok());
    let mut stale = link;
    stale.evidence_revision = 3;
    assert!(stale.validate().is_err());
}
