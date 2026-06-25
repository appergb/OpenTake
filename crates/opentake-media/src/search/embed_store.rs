//! `EmbeddingStore` — per-asset frame embeddings on disk in the `PALMEMB1`
//! binary format, byte-compatible with upstream
//! (`Search/Indexing/EmbeddingStore.swift`). Vectors are f16 on disk, f32 in
//! memory.
//!
//! Layout (all little-endian, unaligned):
//! ```text
//! "PALMEMB1"            8 bytes ASCII magic
//! u32 headerLen        4 bytes
//! JSON(Header)         headerLen bytes
//! count rows, each rowBytes = 24 + dim*2:
//!     f64 time
//!     f64 shotStart
//!     f64 shotEnd
//!     dim × f16
//! ```

use std::path::{Path, PathBuf};

use byteorder::{ByteOrder, LittleEndian};
use half::f16;
use serde::{Deserialize, Serialize};

use crate::cache_key::{file_identity_key, KEY_HEX_LEN};
use crate::error::{MediaError, Result};

/// Magic bytes prefixing every `.embed` file.
pub const MAGIC: &[u8; 8] = b"PALMEMB1";
/// Cache subdirectory (kept identical to upstream).
pub const CACHE_SUBDIR: &str = "Embeddings";

/// Embedding store header (JSON, camelCase to match upstream).
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub model: String,
    pub model_version: i32,
    pub sampler_version: i32,
    pub dim: usize,
    pub count: usize,
}

/// One indexed frame's metadata (the f32 vector lives in `AssetIndex.vectors`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    pub time: f64,
    pub shot_start: f64,
    pub shot_end: f64,
}

/// A loaded index: header, per-row metadata, and a flat `count*dim` f32 vector
/// block (row-major) ready for matrix·vector ranking.
#[derive(Clone, Debug, PartialEq)]
pub struct AssetIndex {
    pub header: Header,
    pub rows: Vec<Row>,
    pub vectors: Vec<f32>,
}

fn embed_path(cache_root: &Path, key: &str) -> PathBuf {
    cache_root.join(CACHE_SUBDIR).join(format!("{key}.embed"))
}

fn row_bytes(dim: usize) -> usize {
    3 * 8 + dim * 2
}

/// Cache key for `path` (`file_identity_key` with 32 hex chars).
pub fn key(path: &Path) -> Option<String> {
    file_identity_key(path, KEY_HEX_LEN)
}

/// Serialize an index to the `PALMEMB1` byte layout. Pure (no IO) so the exact
/// bytes are testable; `save` wraps this with an atomic file write.
pub fn encode(header: &Header, rows: &[Row], vectors: &[f32]) -> Result<Vec<u8>> {
    if rows.len() != header.count || vectors.len() != header.count * header.dim {
        return Err(MediaError::StoreCorrupt);
    }
    let json = serde_json::to_vec(header).map_err(|_| MediaError::StoreCorrupt)?;
    let mut out = Vec::with_capacity(8 + 4 + json.len() + header.count * row_bytes(header.dim));
    out.extend_from_slice(MAGIC);
    let mut len4 = [0u8; 4];
    LittleEndian::write_u32(&mut len4, json.len() as u32);
    out.extend_from_slice(&len4);
    out.extend_from_slice(&json);

    let mut buf8 = [0u8; 8];
    let mut buf2 = [0u8; 2];
    for (i, row) in rows.iter().enumerate() {
        for v in [row.time, row.shot_start, row.shot_end] {
            LittleEndian::write_f64(&mut buf8, v);
            out.extend_from_slice(&buf8);
        }
        for d in 0..header.dim {
            let h = f16::from_f32(vectors[i * header.dim + d]);
            LittleEndian::write_u16(&mut buf2, h.to_bits());
            out.extend_from_slice(&buf2);
        }
    }
    Ok(out)
}

/// Parse the `PALMEMB1` byte layout into an [`AssetIndex`]. Strict length
/// validation: any mismatch → [`MediaError::StoreCorrupt`].
pub fn decode(data: &[u8]) -> Result<AssetIndex> {
    if data.len() < MAGIC.len() + 4 || &data[..MAGIC.len()] != MAGIC {
        return Err(MediaError::StoreCorrupt);
    }
    let mut offset = MAGIC.len();
    let header_len = LittleEndian::read_u32(&data[offset..offset + 4]) as usize;
    offset += 4;
    if data.len() < offset + header_len {
        return Err(MediaError::StoreCorrupt);
    }
    let header: Header = serde_json::from_slice(&data[offset..offset + header_len])
        .map_err(|_| MediaError::StoreCorrupt)?;
    offset += header_len;

    let rb = row_bytes(header.dim);
    if data.len() != offset + header.count * rb {
        return Err(MediaError::StoreCorrupt);
    }

    let mut rows = Vec::with_capacity(header.count);
    let mut vectors = vec![0.0f32; header.count * header.dim];
    for i in 0..header.count {
        let base = offset + i * rb;
        let time = LittleEndian::read_f64(&data[base..base + 8]);
        let shot_start = LittleEndian::read_f64(&data[base + 8..base + 16]);
        let shot_end = LittleEndian::read_f64(&data[base + 16..base + 24]);
        rows.push(Row {
            time,
            shot_start,
            shot_end,
        });
        for d in 0..header.dim {
            let off = base + 24 + d * 2;
            let bits = LittleEndian::read_u16(&data[off..off + 2]);
            vectors[i * header.dim + d] = f16::from_bits(bits).to_f32();
        }
    }
    Ok(AssetIndex {
        header,
        rows,
        vectors,
    })
}

/// Read just the header from a `.embed` file (cheap currency check).
pub fn header(cache_root: &Path, key: &str) -> Option<Header> {
    let data = std::fs::read(embed_path(cache_root, key)).ok()?;
    if data.len() < MAGIC.len() + 4 || &data[..MAGIC.len()] != MAGIC {
        return None;
    }
    let header_len = LittleEndian::read_u32(&data[MAGIC.len()..MAGIC.len() + 4]) as usize;
    let start = MAGIC.len() + 4;
    if data.len() < start + header_len {
        return None;
    }
    serde_json::from_slice(&data[start..start + header_len]).ok()
}

/// True when an on-disk index matches `(model, model_version, sampler_version)`.
pub fn is_current(
    cache_root: &Path,
    key: &str,
    model: &str,
    model_version: i32,
    sampler_version: i32,
) -> bool {
    match header(cache_root, key) {
        Some(h) => {
            h.model == model
                && h.model_version == model_version
                && h.sampler_version == sampler_version
        }
        None => false,
    }
}

/// Load a full index from `<cache_root>/Embeddings/<key>.embed`.
pub fn load(cache_root: &Path, key: &str) -> Result<AssetIndex> {
    let data = std::fs::read(embed_path(cache_root, key))?;
    decode(&data)
}

/// Atomically write an index. Creates the cache subdirectory if needed.
pub fn save(
    cache_root: &Path,
    key: &str,
    header: &Header,
    rows: &[Row],
    vectors: &[f32],
) -> Result<()> {
    let bytes = encode(header, rows, vectors)?;
    let dir = cache_root.join(CACHE_SUBDIR);
    std::fs::create_dir_all(&dir)?;
    let final_path = dir.join(format!("{key}.embed"));
    let tmp_path = dir.join(format!("{key}.embed.tmp"));
    std::fs::write(&tmp_path, &bytes)?;
    std::fs::rename(&tmp_path, &final_path)?;
    Ok(())
}

/// Remove the entire embeddings cache directory.
pub fn clear_all(cache_root: &Path) -> Result<()> {
    let dir = cache_root.join(CACHE_SUBDIR);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_dim2() -> Header {
        Header {
            model: "siglip2-base-patch16-256".into(),
            model_version: 1,
            sampler_version: 1,
            dim: 2,
            count: 2,
        }
    }

    #[test]
    fn encode_starts_with_magic_and_header_len() {
        let h = header_dim2();
        let rows = vec![
            Row {
                time: 0.0,
                shot_start: 0.0,
                shot_end: 1.0,
            },
            Row {
                time: 1.0,
                shot_start: 1.0,
                shot_end: 2.0,
            },
        ];
        let vectors = vec![0.5, -0.5, 1.0, 0.0];
        let bytes = encode(&h, &rows, &vectors).unwrap();
        assert_eq!(&bytes[..8], MAGIC);
        // total = 8 + 4 + json + 2*(24 + 2*2)
        let json_len = LittleEndian::read_u32(&bytes[8..12]) as usize;
        assert_eq!(bytes.len(), 8 + 4 + json_len + 2 * (24 + 4));
    }

    #[test]
    fn encode_decode_roundtrip_f16_quantized() {
        let h = header_dim2();
        let rows = vec![
            Row {
                time: 0.0,
                shot_start: 0.0,
                shot_end: 1.5,
            },
            Row {
                time: 2.25,
                shot_start: 1.5,
                shot_end: 3.0,
            },
        ];
        let vectors = vec![0.5f32, -0.25, 1.0, 0.125];
        let bytes = encode(&h, &rows, &vectors).unwrap();
        let idx = decode(&bytes).unwrap();
        assert_eq!(idx.header, h);
        assert_eq!(idx.rows, rows);
        // f16 round-trip is exact for these dyadic values.
        for (a, b) in vectors.iter().zip(idx.vectors.iter()) {
            assert_eq!(*a, *b);
        }
    }

    #[test]
    fn row_bytes_for_dim768_is_1560() {
        assert_eq!(row_bytes(768), 24 + 768 * 2);
        assert_eq!(row_bytes(768), 1560);
    }

    #[test]
    fn decode_rejects_bad_magic() {
        let mut bytes = vec![0u8; 20];
        bytes[..8].copy_from_slice(b"NOTMAGIC");
        assert!(matches!(decode(&bytes), Err(MediaError::StoreCorrupt)));
    }

    #[test]
    fn decode_rejects_truncation() {
        let h = header_dim2();
        let rows = vec![
            Row {
                time: 0.0,
                shot_start: 0.0,
                shot_end: 1.0,
            },
            Row {
                time: 1.0,
                shot_start: 1.0,
                shot_end: 2.0,
            },
        ];
        let vectors = vec![0.5, -0.5, 1.0, 0.0];
        let mut bytes = encode(&h, &rows, &vectors).unwrap();
        bytes.truncate(bytes.len() - 1);
        assert!(matches!(decode(&bytes), Err(MediaError::StoreCorrupt)));
    }

    #[test]
    fn decode_rejects_extra_trailing_bytes() {
        let h = header_dim2();
        let rows = vec![Row {
            time: 0.0,
            shot_start: 0.0,
            shot_end: 1.0,
        }];
        let mut h2 = h.clone();
        h2.count = 1;
        let vectors = vec![0.5, -0.5];
        let mut bytes = encode(&h2, &rows, &vectors).unwrap();
        bytes.push(0xFF);
        assert!(matches!(decode(&bytes), Err(MediaError::StoreCorrupt)));
    }

    #[test]
    fn encode_rejects_count_vector_mismatch() {
        let h = header_dim2(); // count 2, dim 2 → needs 4 floats
        let rows = vec![Row {
            time: 0.0,
            shot_start: 0.0,
            shot_end: 1.0,
        }]; // only 1 row
        assert!(matches!(
            encode(&h, &rows, &[0.0; 4]),
            Err(MediaError::StoreCorrupt)
        ));
    }

    #[test]
    fn save_load_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let h = header_dim2();
        let rows = vec![
            Row {
                time: 0.0,
                shot_start: 0.0,
                shot_end: 1.0,
            },
            Row {
                time: 1.0,
                shot_start: 1.0,
                shot_end: 2.0,
            },
        ];
        let vectors = vec![0.25f32, 0.5, 0.75, 1.0];
        save(dir.path(), "k", &h, &rows, &vectors).unwrap();
        let idx = load(dir.path(), "k").unwrap();
        assert_eq!(idx.header, h);
        assert_eq!(idx.rows, rows);
    }

    #[test]
    fn is_current_checks_version_triple() {
        let dir = tempfile::tempdir().unwrap();
        let h = header_dim2();
        save(
            dir.path(),
            "k",
            &h,
            &[
                Row {
                    time: 0.0,
                    shot_start: 0.0,
                    shot_end: 1.0,
                },
                Row {
                    time: 1.0,
                    shot_start: 1.0,
                    shot_end: 2.0,
                },
            ],
            &[0.0; 4],
        )
        .unwrap();
        assert!(is_current(
            dir.path(),
            "k",
            "siglip2-base-patch16-256",
            1,
            1
        ));
        assert!(!is_current(dir.path(), "k", "other-model", 1, 1));
        assert!(!is_current(
            dir.path(),
            "k",
            "siglip2-base-patch16-256",
            2,
            1
        ));
        assert!(!is_current(
            dir.path(),
            "k",
            "siglip2-base-patch16-256",
            1,
            2
        ));
        assert!(!is_current(
            dir.path(),
            "missing",
            "siglip2-base-patch16-256",
            1,
            1
        ));
    }

    #[test]
    fn clear_all_removes_directory() {
        let dir = tempfile::tempdir().unwrap();
        let h = header_dim2();
        save(
            dir.path(),
            "k",
            &h,
            &[
                Row {
                    time: 0.0,
                    shot_start: 0.0,
                    shot_end: 1.0,
                },
                Row {
                    time: 1.0,
                    shot_start: 1.0,
                    shot_end: 2.0,
                },
            ],
            &[0.0; 4],
        )
        .unwrap();
        assert!(dir.path().join(CACHE_SUBDIR).exists());
        clear_all(dir.path()).unwrap();
        assert!(!dir.path().join(CACHE_SUBDIR).exists());
    }
}
