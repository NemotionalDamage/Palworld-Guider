use guide_core::{AnswerStatus, GuideEngine};

const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reviewed");

#[test]
fn matching_configured_version_produces_no_warning() {
    let engine = GuideEngine::load_directory(DATA_DIRECTORY, Some("1.0".to_string()))
        .expect("dataset loads");
    let answer = engine.lookup_item("stone");
    assert_eq!(answer.status, AnswerStatus::Ok);
    assert!(answer.version.matches);
    assert!(!answer
        .uncertainty
        .iter()
        .any(|u| u.contains("does not match configured game version")));
}

#[test]
fn mismatched_configured_version_produces_warning() {
    let engine = GuideEngine::load_directory(DATA_DIRECTORY, Some("0.1".to_string()))
        .expect("dataset loads");
    let answer = engine.lookup_item("stone");
    assert!(!answer.version.matches);
    assert!(answer
        .uncertainty
        .iter()
        .any(|u| u.contains("does not match configured game version")));
}

#[test]
fn none_configured_version_matches_without_warning() {
    let engine = GuideEngine::load_directory(DATA_DIRECTORY, None).expect("dataset loads");
    let answer = engine.lookup_item("stone");
    assert!(answer.version.matches);
    assert!(!answer
        .uncertainty
        .iter()
        .any(|u| u.contains("does not match configured game version")));
}

#[test]
fn version_info_present_in_every_answer() {
    let engine = GuideEngine::load_directory(DATA_DIRECTORY, Some("1.0".to_string()))
        .expect("dataset loads");

    let item = engine.lookup_item("stone");
    assert_eq!(item.version.knowledge_version, "1.0");

    let pal = engine.lookup_pal("lamball");
    assert_eq!(pal.version.knowledge_version, "1.0");

    let breeding = engine.calculate_breeding_result("lamball", "lamball");
    assert_ne!(breeding.version.knowledge_version, "");
    assert_eq!(
        breeding.version.configured_game_version,
        Some("1.0".to_string())
    );
}

#[test]
fn version_mismatch_visible_in_calculator_answer() {
    let engine = GuideEngine::load_directory(DATA_DIRECTORY, Some("0.5".to_string()))
        .expect("dataset loads");
    let answer = engine.calculate_materials("stone", 1);
    assert!(!answer.version.matches);
    assert!(answer
        .uncertainty
        .iter()
        .any(|u| u.contains("does not match configured game version")));
}

#[test]
fn version_mismatch_visible_in_breeding_answer() {
    let engine = GuideEngine::load_directory(DATA_DIRECTORY, Some("2.0".to_string()))
        .expect("dataset loads");
    let answer = engine.calculate_breeding_result("lamball", "lamball");
    assert!(!answer.version.matches);
    assert!(answer
        .uncertainty
        .iter()
        .any(|u| u.contains("does not match configured game version")));
}

#[test]
fn configured_version_propagates_to_all_answer_types() {
    let engine = GuideEngine::load_directory(DATA_DIRECTORY, Some("1.0".to_string()))
        .expect("dataset loads");

    let item = engine.lookup_item("stone");
    assert_eq!(
        item.version.configured_game_version,
        Some("1.0".to_string())
    );

    let pal = engine.lookup_pal("lamball");
    assert_eq!(pal.version.configured_game_version, Some("1.0".to_string()));

    let tech = engine.lookup_technology("level 2");
    assert_eq!(
        tech.version.configured_game_version,
        Some("1.0".to_string())
    );
}
