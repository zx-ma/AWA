use awa_core::enrollment::store::EnrollmentStore;
use awa_core::enrollment::{EnrollmentFile, MAX_SAMPLES_PER_LABEL};
use awa_core::pipeline::arcface::EMBEDDING_DIM;

fn dummy_embedding(seed: f32) -> [f32; EMBEDDING_DIM] {
    let mut e = [0.0_f32; EMBEDDING_DIM];
    for (i, v) in e.iter_mut().enumerate() {
        *v = (i as f32 * 0.001 + seed).sin();
    }
    let norm = e.iter().map(|x| x * x).sum::<f32>().sqrt();
    for v in e.iter_mut() {
        *v /= norm;
    }
    e
}

#[test]
fn load_returns_none_for_missing_user() {
    let dir = tempfile::tempdir().unwrap();
    let store = EnrollmentStore::new(dir.path());
    let result = store.load("nobody").unwrap();
    assert!(result.is_none());
}

#[test]
fn save_and_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = EnrollmentStore::new(dir.path());

    let mut file = EnrollmentFile::new("alice");
    file.records.push(awa_core::enrollment::EnrollmentRecord {
        label: "primary".into(),
        samples: vec![],
        created_at: chrono::Utc::now(),
    });
    store.save(&file).unwrap();

    let loaded = store.load("alice").unwrap().expect("file exists");
    assert_eq!(loaded.username, "alice");
    assert_eq!(loaded.records.len(), 1);
    assert_eq!(loaded.records[0].label, "primary");
}

#[test]
fn add_sample_creates_record_and_appends() {
    let dir = tempfile::tempdir().unwrap();
    let store = EnrollmentStore::new(dir.path());

    store
        .add_sample("alice", "primary", dummy_embedding(0.1), "test")
        .unwrap();
    store
        .add_sample("alice", "primary", dummy_embedding(0.2), "test")
        .unwrap();
    store
        .add_sample("alice", "with_glasses", dummy_embedding(0.3), "test")
        .unwrap();

    let loaded = store.load("alice").unwrap().unwrap();
    assert_eq!(loaded.records.len(), 2);

    let primary = loaded
        .records
        .iter()
        .find(|r| r.label == "primary")
        .unwrap();
    assert_eq!(primary.samples.len(), 2);

    let glasses = loaded
        .records
        .iter()
        .find(|r| r.label == "with_glasses")
        .unwrap();
    assert_eq!(glasses.samples.len(), 1);
}

#[test]
fn add_sample_prunes_when_exceeds_max() {
    let dir = tempfile::tempdir().unwrap();
    let store = EnrollmentStore::new(dir.path());

    for i in 0..10 {
        store
            .add_sample("alice", "primary", dummy_embedding(i as f32), "test")
            .unwrap();
    }

    let loaded = store.load("alice").unwrap().unwrap();
    assert_eq!(loaded.records[0].samples.len(), MAX_SAMPLES_PER_LABEL);
}

#[test]
fn best_similarity_returns_max_across_samples() {
    let dir = tempfile::tempdir().unwrap();
    let store = EnrollmentStore::new(dir.path());

    let target = dummy_embedding(0.5);
    let other = dummy_embedding(99.0);
    store.add_sample("alice", "primary", other, "test").unwrap();
    store
        .add_sample("alice", "primary", target, "test")
        .unwrap();

    let sim = store.best_similarity("alice", &target).unwrap().unwrap();
    assert!(
        (sim - 1.0).abs() < 1e-4,
        "self-match should be 1.0, got {}",
        sim
    );

    let none = store.best_similarity("nobody", &target).unwrap();
    assert!(none.is_none());
}
