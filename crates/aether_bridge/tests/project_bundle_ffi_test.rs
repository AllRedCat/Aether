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

#[test]
fn test_import_media_file_into_project_bundle_with_active_timeline() {
    use aether_bridge::api::{create_new_timeline, get_media_items, import_media_file};
    use std::io::Write;

    let _guard = TEST_MUTEX.lock().unwrap();
    let tmp = TestDir::new("bundle_media_import");
    let bundle_path = tmp.path.join("testando.aether");
    let bundle_path_str = bundle_path.to_str().unwrap().to_string();
    let db_path_str = bundle_path.join("project.db").to_str().unwrap().to_string();

    // 1. Create bundle (WelcomeScreen flow)
    let _meta = create_project_bundle(bundle_path_str.clone(), "testando".to_string())
        .expect("create_project_bundle failed");

    // 2. Initialize default timeline in SQLite (EditorScreen / activeTimelineProvider flow)
    let _tl_state = create_new_timeline("Timeline 1".to_string(), 1920, 1080, 60, 1)
        .expect("create_new_timeline failed");

    // 3. Create a valid test WAV file and import it via import_media_file passing project.db path
    let wav_path = tmp.path.join("gravacao_show.wav");
    {
        let sample_rate: u32 = 48_000;
        let num_samples: usize = 2048;
        let data_size = (num_samples * 2) as u32;
        let riff_size = 36 + data_size;
        let mut f = fs::File::create(&wav_path).unwrap();
        f.write_all(b"RIFF").unwrap();
        f.write_all(&riff_size.to_le_bytes()).unwrap();
        f.write_all(b"WAVEfmt ").unwrap();
        f.write_all(&16u32.to_le_bytes()).unwrap();
        f.write_all(&1u16.to_le_bytes()).unwrap();
        f.write_all(&1u16.to_le_bytes()).unwrap();
        f.write_all(&sample_rate.to_le_bytes()).unwrap();
        f.write_all(&(sample_rate * 2).to_le_bytes()).unwrap();
        f.write_all(&2u16.to_le_bytes()).unwrap();
        f.write_all(&16u16.to_le_bytes()).unwrap();
        f.write_all(b"data").unwrap();
        f.write_all(&data_size.to_le_bytes()).unwrap();
        for _ in 0..num_samples {
            f.write_all(&1000i16.to_le_bytes()).unwrap();
        }
    }

    let imported = import_media_file(
        Some(db_path_str.clone()),
        wav_path.to_str().unwrap().to_string(),
    )
    .expect("import_media_file into bundle project.db should succeed");

    assert_eq!(imported.file_name, "gravacao_show.wav");

    // 4. Verify media item is persisted and retrievable via get_media_items
    let items = get_media_items(db_path_str).expect("get_media_items should succeed");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, imported.id);
    assert_eq!(items[0].file_name, "gravacao_show.wav");
}
