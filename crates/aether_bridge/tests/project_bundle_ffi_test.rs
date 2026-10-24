use std::fs;
use std::sync::Mutex;
use uuid::Uuid;

use aether_bridge::api::{
    create_project_bundle, get_active_bundle_path, open_project_bundle,
};

static TEST_MUTEX: Mutex<()> = Mutex::new(());

struct TestDir {
    path: std::path::PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("aether_bundle_test_{}_{}", name, Uuid::new_v4()));
        fs::create_dir_all(&p).unwrap();
        Self { path: p }
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn test_create_and_open_project_bundle_lifecycle() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let tmp = TestDir::new("lifecycle");
    let bundle_path = tmp.path.join("MeuProjeto.aether");
    let bundle_path_str = bundle_path.to_str().unwrap().to_string();

    // 1. Create project bundle
    let meta = create_project_bundle(bundle_path_str.clone(), "Meu Projeto de Teste".to_string())
        .expect("create_project_bundle failed");

    assert_eq!(meta.name, "Meu Projeto de Teste");
    assert!(!meta.id.is_empty());
    assert!(!meta.created_at.is_empty());
    assert!(!meta.updated_at.is_empty());

    // Verify tree on disk
    assert!(bundle_path.is_dir());
    assert!(bundle_path.join("project.db").is_file());
    assert!(bundle_path.join("cache/index").is_dir());
    assert!(bundle_path.join("cache/peaks").is_dir());
    assert!(bundle_path.join("cache/thumbs").is_dir());

    // Verify active bundle tracking
    assert_eq!(
        get_active_bundle_path(),
        Some(bundle_path.to_string_lossy().to_string())
    );

    // 2. Open project bundle
    let opened_meta = open_project_bundle(bundle_path_str.clone())
        .expect("open_project_bundle failed");

    assert_eq!(opened_meta.id, meta.id);
    assert_eq!(opened_meta.name, "Meu Projeto de Teste");
    assert_eq!(opened_meta.created_at, meta.created_at);
    assert_eq!(opened_meta.updated_at, meta.updated_at);
}

#[test]
fn test_open_project_bundle_recreates_cleared_cache_directories() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let tmp = TestDir::new("cache_recreate");
    let bundle_path = tmp.path.join("CacheClearTest.aether");
    let bundle_path_str = bundle_path.to_str().unwrap().to_string();

    // 1. Create bundle
    let _ = create_project_bundle(bundle_path_str.clone(), "Cache Test".to_string())
        .expect("create failed");

    let cache_dir = bundle_path.join("cache");
    assert!(cache_dir.is_dir());

    // 2. Simulate user clearing the entire cache directory
    fs::remove_dir_all(&cache_dir).expect("failed to remove cache dir");
    assert!(!cache_dir.exists());

    // 3. Open project bundle must recreate all cache subdirectories
    let opened = open_project_bundle(bundle_path_str).expect("open failed");
    assert_eq!(opened.name, "Cache Test");

    assert!(bundle_path.join("cache/index").is_dir());
    assert!(bundle_path.join("cache/peaks").is_dir());
    assert!(bundle_path.join("cache/thumbs").is_dir());
}

#[test]
fn test_open_nonexistent_bundle_returns_error() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let res = open_project_bundle("/nonexistent/path/NoSuchProject.aether".to_string());
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.contains("does not exist"), "Error was: {}", err);
}

#[test]
fn test_create_project_bundle_normalizes_extension() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let tmp = TestDir::new("norm");
    // Pass path without .aether extension
    let raw_path = tmp.path.join("RawProjectName");
    let raw_path_str = raw_path.to_str().unwrap().to_string();

    let meta = create_project_bundle(raw_path_str, "Raw Project".to_string())
        .expect("create failed");
    assert_eq!(meta.name, "Raw Project");

    // The created directory must have .aether
    let expected_bundle = tmp.path.join("RawProjectName.aether");
    assert!(expected_bundle.is_dir());
    assert!(expected_bundle.join("project.db").is_file());
}
