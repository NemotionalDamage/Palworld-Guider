use game_knowledge::KnowledgeStore;
use knowledge_index::{IndexStatus, KnowledgeIndex};

const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reviewed");

fn index() -> KnowledgeIndex {
    let store = KnowledgeStore::load_directory(DATA_DIRECTORY).expect("dataset loads");
    KnowledgeIndex::from_store(&store, Some("1.0.3".to_string())).expect("index builds")
}

#[test]
fn search_wood_returns_relevant_items() {
    let index = index();
    let answer = index.search("wood", 10);
    assert_eq!(answer.status, IndexStatus::Ok);
    assert!(!answer.results.is_empty());
    assert!(answer.results.iter().any(|r| r.record_id == "ITEM_WOOD"));
}

#[test]
fn search_sphere_returns_pal_sphere() {
    let index = index();
    let answer = index.search("sphere", 10);
    assert_eq!(answer.status, IndexStatus::Ok);
    assert!(!answer.results.is_empty());
    assert!(answer
        .results
        .iter()
        .any(|r| r.record_id == "ITEM_PAL_SPHERE"));
}

#[test]
fn search_nonexistent_term_returns_unknown() {
    let index = index();
    let answer = index.search("xyzzy-nonexistent", 10);
    assert_eq!(answer.status, IndexStatus::Unknown);
    assert!(answer.results.is_empty());
}

#[test]
fn search_respects_result_limit() {
    let index = index();
    let answer = index.search("wood", 1);
    assert!(answer.results.len() <= 1);
}

#[test]
fn search_empty_query_returns_unknown() {
    let index = index();
    let answer = index.search("", 10);
    assert_eq!(answer.status, IndexStatus::Unknown);
}

#[test]
fn search_returns_provenance() {
    let index = index();
    let answer = index.search("wood", 10);
    if answer.status == IndexStatus::Ok {
        assert!(answer
            .results
            .iter()
            .all(|r| !r.provenance.source_id.is_empty()));
    }
}

#[test]
fn search_returns_version_info() {
    let index = index();
    let answer = index.search("wood", 10);
    assert_eq!(answer.version.knowledge_version, "1.0.3");
    assert!(answer.version.matches);
}
