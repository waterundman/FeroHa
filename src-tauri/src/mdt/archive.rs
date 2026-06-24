use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::mdt::indexer::index_vault_with_artifacts;
use crate::mdt::types::MdtProjectIndex;
use crate::parser::frontmatter::parse_frontmatter;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MdtArchiveManifest {
    pub mdtz_version: String,
    pub format: String,
    pub root_manifest: String,
    pub generated_at: u64,
    pub project: MdtProjectIndex,
    pub files: Vec<MdtArchiveFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MdtArchiveFile {
    pub path: String,
    pub kind: String,
    pub bytes: u64,
    pub content_hash: String,
}

struct ZipEntry {
    name: String,
    data: Vec<u8>,
}

pub fn pack_mdtz(project_root: &Path, archive_path: &Path) -> Result<MdtArchiveManifest, String> {
    if !project_root.is_dir() {
        return Err(format!(
            "project root is not a directory: {}",
            project_root.display()
        ));
    }

    let project = index_vault_with_artifacts(project_root)?;
    validate_declared_content_hashes(project_root, &project)?;

    let mut file_paths = Vec::new();
    collect_archive_files(project_root, project_root, &mut file_paths)?;
    file_paths.sort();

    let mut manifest_files = Vec::new();
    let mut zip_entries = Vec::new();
    for path in file_paths {
        let relative = normalize_relative_path(project_root, &path);
        if relative == "manifest.json" || relative.ends_with(".mdtz") {
            continue;
        }
        let data =
            fs::read(&path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        manifest_files.push(MdtArchiveFile {
            path: relative.clone(),
            kind: archive_file_kind(&relative),
            bytes: data.len() as u64,
            content_hash: sha256_hex(&data),
        });
        zip_entries.push(ZipEntry {
            name: relative,
            data,
        });
    }

    let manifest = MdtArchiveManifest {
        mdtz_version: "0.1.0".to_string(),
        format: "zip-store".to_string(),
        root_manifest: "manifest.json".to_string(),
        generated_at: current_timestamp_millis(),
        project,
        files: manifest_files,
    };

    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|err| format!("failed to serialize archive manifest: {err}"))?;
    zip_entries.insert(
        0,
        ZipEntry {
            name: "manifest.json".to_string(),
            data: manifest_bytes,
        },
    );
    write_zip_store(archive_path, &zip_entries)?;
    Ok(manifest)
}

pub fn unpack_mdtz(archive_path: &Path, output_root: &Path) -> Result<MdtArchiveManifest, String> {
    let archive_bytes = fs::read(archive_path)
        .map_err(|err| format!("failed to read {}: {err}", archive_path.display()))?;
    let entries = read_zip_store(&archive_bytes)?;
    let manifest_bytes = entries
        .get("manifest.json")
        .ok_or_else(|| "missing manifest.json in .mdtz archive".to_string())?;
    let manifest: MdtArchiveManifest = serde_json::from_slice(manifest_bytes)
        .map_err(|err| format!("failed to parse manifest.json: {err}"))?;

    if manifest.root_manifest != "manifest.json" {
        return Err(format!(
            "invalid root manifest: expected manifest.json, got {}",
            manifest.root_manifest
        ));
    }

    let manifest_path = output_root.join("manifest.json");
    let mut pending_writes: Vec<(PathBuf, Vec<u8>)> = vec![(manifest_path, manifest_bytes.clone())];
    for file in &manifest.files {
        let safe_path = safe_archive_path(&file.path)?;
        let data = entries
            .get(&file.path)
            .ok_or_else(|| format!("manifest entry missing from archive: {}", file.path))?;
        let actual_hash = sha256_hex(data);
        if actual_hash != file.content_hash {
            return Err(format!(
                "content_hash mismatch for {}: expected {}, got {}",
                file.path, file.content_hash, actual_hash
            ));
        }
        if data.len() as u64 != file.bytes {
            return Err(format!(
                "byte length mismatch for {}: expected {}, got {}",
                file.path,
                file.bytes,
                data.len()
            ));
        }
        pending_writes.push((output_root.join(safe_path), data.clone()));
    }

    for (path, data) in pending_writes {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
        }
        fs::write(&path, data)
            .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    }

    Ok(manifest)
}

fn validate_declared_content_hashes(
    project_root: &Path,
    project: &MdtProjectIndex,
) -> Result<(), String> {
    for node in &project.nodes {
        let Some(expected_hash) = node.content_hash.as_ref() else {
            continue;
        };
        let path = project_root.join(&node.path);
        let content = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let actual_hash = mdt_body_hash(&content);
        if actual_hash != *expected_hash {
            return Err(format!(
                "content_hash mismatch for {}: expected {}, got {}",
                node.path, expected_hash, actual_hash
            ));
        }
    }
    Ok(())
}

fn mdt_body_hash(content: &str) -> String {
    let body = parse_frontmatter(content)
        .map(|(_, offset)| &content[offset..])
        .unwrap_or(content);
    sha256_hex(body.as_bytes())
}

fn collect_archive_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    for entry in fs::read_dir(current)
        .map_err(|err| format!("failed to read directory {}: {err}", current.display()))?
    {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|part| part.to_str())
            .unwrap_or("");
        if path.is_dir() {
            if should_skip_archive_dir(name) {
                continue;
            }
            collect_archive_files(root, &path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    let _ = root;
    Ok(())
}

fn should_skip_archive_dir(name: &str) -> bool {
    matches!(name, ".git" | "node_modules" | "target" | "dist" | "build")
}

fn archive_file_kind(path: &str) -> String {
    if path.ends_with(".md") || path.ends_with(".mdt") {
        "node".to_string()
    } else if path.contains("/indexes/") || path.ends_with("/indexes") {
        "index".to_string()
    } else if path.starts_with("assets/") || path.contains("/assets/") {
        "asset".to_string()
    } else if path.starts_with("skills/") || path.contains("/skills/") {
        "skill".to_string()
    } else if path.starts_with("logs/") || path.contains("/logs/") {
        "log".to_string()
    } else {
        "file".to_string()
    }
}

fn normalize_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn safe_archive_path(path: &str) -> Result<PathBuf, String> {
    let path = path.replace('\\', "/");
    if path.trim().is_empty() {
        return Err("archive path is empty".to_string());
    }
    let mut safe = PathBuf::new();
    for component in Path::new(&path).components() {
        match component {
            Component::Normal(part) => safe.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("unsafe archive path: {path}"));
            }
        }
    }
    Ok(safe)
}

fn current_timestamp_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn write_zip_store(path: &Path, entries: &[ZipEntry]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }

    let mut cursor = Cursor::new(Vec::new());
    let mut central_records = Vec::new();

    for entry in entries {
        let name_bytes = entry.name.as_bytes();
        if name_bytes.len() > u16::MAX as usize {
            return Err(format!("zip entry name is too long: {}", entry.name));
        }
        if entry.data.len() > u32::MAX as usize {
            return Err(format!("zip entry is too large: {}", entry.name));
        }
        let local_offset = cursor.position() as u32;
        let crc = crc32(&entry.data);
        let size = entry.data.len() as u32;

        write_u32(&mut cursor, 0x0403_4b50)?;
        write_u16(&mut cursor, 20)?;
        write_u16(&mut cursor, 0)?;
        write_u16(&mut cursor, 0)?;
        write_u16(&mut cursor, 0)?;
        write_u16(&mut cursor, 0)?;
        write_u32(&mut cursor, crc)?;
        write_u32(&mut cursor, size)?;
        write_u32(&mut cursor, size)?;
        write_u16(&mut cursor, name_bytes.len() as u16)?;
        write_u16(&mut cursor, 0)?;
        cursor
            .write_all(name_bytes)
            .map_err(|err| err.to_string())?;
        cursor
            .write_all(&entry.data)
            .map_err(|err| err.to_string())?;

        central_records.push((entry.name.clone(), crc, size, local_offset));
    }

    let central_offset = cursor.position() as u32;
    for (name, crc, size, local_offset) in &central_records {
        let name_bytes = name.as_bytes();
        write_u32(&mut cursor, 0x0201_4b50)?;
        write_u16(&mut cursor, 20)?;
        write_u16(&mut cursor, 20)?;
        write_u16(&mut cursor, 0)?;
        write_u16(&mut cursor, 0)?;
        write_u16(&mut cursor, 0)?;
        write_u16(&mut cursor, 0)?;
        write_u32(&mut cursor, *crc)?;
        write_u32(&mut cursor, *size)?;
        write_u32(&mut cursor, *size)?;
        write_u16(&mut cursor, name_bytes.len() as u16)?;
        write_u16(&mut cursor, 0)?;
        write_u16(&mut cursor, 0)?;
        write_u16(&mut cursor, 0)?;
        write_u16(&mut cursor, 0)?;
        write_u32(&mut cursor, 0)?;
        write_u32(&mut cursor, *local_offset)?;
        cursor
            .write_all(name_bytes)
            .map_err(|err| err.to_string())?;
    }
    let central_size = cursor.position() as u32 - central_offset;

    if central_records.len() > u16::MAX as usize {
        return Err("too many zip entries for .mdtz archive".to_string());
    }
    write_u32(&mut cursor, 0x0605_4b50)?;
    write_u16(&mut cursor, 0)?;
    write_u16(&mut cursor, 0)?;
    write_u16(&mut cursor, central_records.len() as u16)?;
    write_u16(&mut cursor, central_records.len() as u16)?;
    write_u32(&mut cursor, central_size)?;
    write_u32(&mut cursor, central_offset)?;
    write_u16(&mut cursor, 0)?;

    fs::write(path, cursor.into_inner())
        .map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn read_zip_store(bytes: &[u8]) -> Result<HashMap<String, Vec<u8>>, String> {
    let eocd_offset = find_eocd(bytes).ok_or_else(|| "invalid zip: missing EOCD".to_string())?;
    let entry_count = read_u16_at(bytes, eocd_offset + 10)? as usize;
    let central_offset = read_u32_at(bytes, eocd_offset + 16)? as usize;
    let mut offset = central_offset;
    let mut entries = HashMap::new();

    for _ in 0..entry_count {
        if read_u32_at(bytes, offset)? != 0x0201_4b50 {
            return Err("invalid zip: malformed central directory".to_string());
        }
        let method = read_u16_at(bytes, offset + 10)?;
        if method != 0 {
            return Err("unsupported .mdtz compression method".to_string());
        }
        let crc = read_u32_at(bytes, offset + 16)?;
        let compressed_size = read_u32_at(bytes, offset + 20)? as usize;
        let uncompressed_size = read_u32_at(bytes, offset + 24)? as usize;
        let name_len = read_u16_at(bytes, offset + 28)? as usize;
        let extra_len = read_u16_at(bytes, offset + 30)? as usize;
        let comment_len = read_u16_at(bytes, offset + 32)? as usize;
        let local_offset = read_u32_at(bytes, offset + 42)? as usize;
        let name_start = offset + 46;
        let name_end = name_start + name_len;
        let name = std::str::from_utf8(slice_at(bytes, name_start, name_len)?)
            .map_err(|err| format!("invalid zip entry name: {err}"))?
            .to_string();

        if compressed_size != uncompressed_size {
            return Err(format!("unsupported compressed entry: {name}"));
        }
        let data = read_local_zip_entry(bytes, local_offset, compressed_size)?;
        let actual_crc = crc32(&data);
        if actual_crc != crc {
            return Err(format!("crc mismatch for {name}"));
        }
        entries.insert(name, data);
        offset = name_end + extra_len + comment_len;
    }

    Ok(entries)
}

fn read_local_zip_entry(bytes: &[u8], offset: usize, size: usize) -> Result<Vec<u8>, String> {
    if read_u32_at(bytes, offset)? != 0x0403_4b50 {
        return Err("invalid zip: malformed local header".to_string());
    }
    let name_len = read_u16_at(bytes, offset + 26)? as usize;
    let extra_len = read_u16_at(bytes, offset + 28)? as usize;
    let data_start = offset + 30 + name_len + extra_len;
    Ok(slice_at(bytes, data_start, size)?.to_vec())
}

fn find_eocd(bytes: &[u8]) -> Option<usize> {
    let min = bytes.len().saturating_sub(66_000);
    (min..bytes.len().saturating_sub(3))
        .rev()
        .find(|offset| bytes[*offset..*offset + 4] == [0x50, 0x4b, 0x05, 0x06])
}

fn write_u16(cursor: &mut Cursor<Vec<u8>>, value: u16) -> Result<(), String> {
    cursor
        .write_all(&value.to_le_bytes())
        .map_err(|err| err.to_string())
}

fn write_u32(cursor: &mut Cursor<Vec<u8>>, value: u32) -> Result<(), String> {
    cursor
        .write_all(&value.to_le_bytes())
        .map_err(|err| err.to_string())
}

fn read_u16_at(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let slice = slice_at(bytes, offset, 2)?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32_at(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let slice = slice_at(bytes, offset, 4)?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn slice_at(bytes: &[u8], offset: usize, len: usize) -> Result<&[u8], String> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| "invalid zip: truncated data".to_string())?;
    bytes
        .get(offset..end)
        .ok_or_else(|| "invalid zip: truncated data".to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    fn pack_unpack_roundtrip_writes_manifest_and_verifies_hashes() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        let unpacked = temp.path().join("unpacked");
        fs::create_dir_all(&vault).unwrap();
        fs::write(
            vault.join("root.md"),
            "---\nmdt_version: \"0.1.0\"\nid: root\ntitle: Root\nlinks:\n  - target: child\n    type: related\n---\n# Root\n\nBody\n",
        )
        .unwrap();
        fs::write(
            vault.join("child.md"),
            "---\nmdt_version: \"0.1.0\"\nid: child\ntitle: Child\n---\n# Child\n",
        )
        .unwrap();

        let archive = temp.path().join("vault.mdtz");
        let manifest = crate::mdt::archive::pack_mdtz(&vault, &archive).unwrap();

        assert!(archive.exists());
        assert_eq!(manifest.root_manifest, "manifest.json");
        assert!(manifest.files.iter().any(|file| file.path == "root.md"));
        assert!(manifest
            .files
            .iter()
            .any(|file| file.path == ".dualtrack/mdt/indexes/nodes.json"));

        let unpacked_manifest = crate::mdt::archive::unpack_mdtz(&archive, &unpacked).unwrap();

        assert_eq!(unpacked_manifest.files.len(), manifest.files.len());
        assert_eq!(
            fs::read_to_string(unpacked.join("root.md")).unwrap(),
            fs::read_to_string(vault.join("root.md")).unwrap()
        );
        assert!(unpacked
            .join(".dualtrack")
            .join("mdt")
            .join("indexes")
            .join("edges.json")
            .exists());
    }

    #[test]
    fn pack_rejects_frontmatter_content_hash_mismatch() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        fs::write(
            vault.join("bad.md"),
            "---\nmdt_version: \"0.1.0\"\nid: bad\ncontent_hash: not-the-real-hash\n---\n# Bad\n",
        )
        .unwrap();

        let archive = temp.path().join("bad.mdtz");
        let error = crate::mdt::archive::pack_mdtz(&vault, &archive).unwrap_err();

        assert!(error.contains("content_hash mismatch"));
    }
}
