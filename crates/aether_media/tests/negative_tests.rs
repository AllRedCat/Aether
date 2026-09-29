use aether_media::{inspect_media_file, MediaError};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_inspect_empty_0byte_file_fails() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("empty.mp4");
    fs::write(&file_path, b"").unwrap();

    let result = inspect_media_file(file_path.to_str().unwrap());

    match result {
        Err(MediaError::EmptyFile(path)) => {
            assert!(path.contains("empty.mp4"));
        }
        other => panic!("Expected MediaError::EmptyFile, got: {:?}", other),
    }
}

#[test]
fn test_inspect_nonexistent_file_fails() {
    let non_existent = "/tmp/aether_tests/definitely_not_a_real_file_12345.mp4";
    let result = inspect_media_file(non_existent);

    match result {
        Err(MediaError::FileNotFound(path)) => {
            assert_eq!(path, non_existent);
        }
        other => panic!("Expected MediaError::FileNotFound, got: {:?}", other),
    }
}

#[test]
fn test_inspect_directory_fails() {
    let dir = tempdir().unwrap();
    let dir_path = dir.path().to_str().unwrap();

    let result = inspect_media_file(dir_path);

    match result {
        Err(MediaError::NotAFile(path)) => {
            assert_eq!(path, dir_path);
        }
        other => panic!("Expected MediaError::NotAFile, got: {:?}", other),
    }
}

#[test]
fn test_inspect_corrupted_header_fails() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("broken_header.png");
    // Write fake PNG signature followed by completely corrupt data
    let mut corrupt_bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    corrupt_bytes.extend_from_slice(b"CORRUPTED_IHDR_GARBAGE_PAYLOAD");
    fs::write(&file_path, corrupt_bytes).unwrap();

    let result = inspect_media_file(file_path.to_str().unwrap());

    match result {
        Err(MediaError::CorruptFile(_)) => {
            // Successfully identified as corrupt header
        }
        other => panic!("Expected MediaError::CorruptFile, got: {:?}", other),
    }
}

#[test]
fn test_inspect_unsupported_plain_text_fails() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("notes.txt");
    fs::write(&file_path, b"Hello world, this is a plain text file.").unwrap();

    let result = inspect_media_file(file_path.to_str().unwrap());

    match result {
        Err(MediaError::UnsupportedFormat(msg)) => {
            assert!(msg.to_lowercase().contains("unsupported") || msg.to_lowercase().contains(".txt"));
        }
        other => panic!("Expected UnsupportedFormat, got: {:?}", other),
    }
}

#[test]
fn test_inspect_unsupported_binary_payload_fails() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("random.dat");
    fs::write(&file_path, &[0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04]).unwrap();

    let result = inspect_media_file(file_path.to_str().unwrap());

    assert!(matches!(result, Err(MediaError::UnsupportedFormat(_))));
}
