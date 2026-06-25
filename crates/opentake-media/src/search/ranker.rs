//! Visual-search ranking — pure function, port of
//! `Search/Query/VisualSearch.swift` (`cblas_sgemv` + best-per-shot + cutoff).
//!
//! For each asset, scores are the dot product of its (count×dim) vector block
//! with the query (the same as the BLAS `sgemv`). Within an asset only the
//! best-scoring frame per shot survives (so one scene cannot flood results).
//! Hits are sorted by score; an optional absolute `min_score` filter runs first;
//! finally the top results are cut to `limit` **and then** filtered by a relative
//! floor (`top * relative_cutoff`) — order matters: limit before floor, so the
//! result is always ≤ `limit`.

use std::collections::HashMap;

use crate::search::embed_store::AssetIndex;

/// One search hit.
#[derive(Clone, PartialEq, Debug)]
pub struct Hit {
    pub asset_id: String,
    pub time: f64,
    pub shot_start: f64,
    pub shot_end: f64,
    pub score: f32,
}

/// Dot product of a flat `count×dim` row-major matrix with `query` (len `dim`).
/// Returns `count` scores. This is the pure-Rust stand-in for `cblas_sgemv`.
fn matvec(vectors: &[f32], count: usize, dim: usize, query: &[f32]) -> Vec<f32> {
    let mut scores = vec![0.0f32; count];
    for (i, slot) in scores.iter_mut().enumerate() {
        let row = &vectors[i * dim..(i + 1) * dim];
        let mut acc = 0.0f32;
        for d in 0..dim {
            acc += row[d] * query[d];
        }
        *slot = acc;
    }
    scores
}

/// Quantize a shot-start time to a stable hash key (bit pattern of the f64).
fn shot_key(t: f64) -> u64 {
    t.to_bits()
}

/// Rank `query` against the given `(asset_id, index)` pairs.
///
/// - `limit`: max hits returned (upstream default 20).
/// - `relative_cutoff`: keep only hits scoring ≥ `top * relative_cutoff`
///   (upstream default 0.85), applied **after** the limit cut.
/// - `min_score`: optional absolute floor (upstream `visualMatchCosineFloor`
///   0.05), applied **before** the top/limit logic.
pub fn search(
    query: &[f32],
    indexes: &[(String, AssetIndex)],
    limit: usize,
    relative_cutoff: f32,
    min_score: Option<f32>,
) -> Vec<Hit> {
    let mut hits: Vec<Hit> = Vec::new();

    for (asset_id, index) in indexes {
        let dim = index.header.dim;
        let count = index.header.count;
        if dim != query.len() || count == 0 {
            continue;
        }
        let scores = matvec(&index.vectors, count, dim, query);

        // Best frame per shot: first occurrence wins ties (existing.score >= score skips).
        let mut best_per_shot: HashMap<u64, (usize, f32)> = HashMap::new();
        for (i, &score) in scores.iter().enumerate() {
            let key = shot_key(index.rows[i].shot_start);
            match best_per_shot.get(&key) {
                Some(&(_, existing)) if existing >= score => {}
                _ => {
                    best_per_shot.insert(key, (i, score));
                }
            }
        }
        for (_, (row_idx, score)) in best_per_shot {
            let row = index.rows[row_idx];
            hits.push(Hit {
                asset_id: asset_id.clone(),
                time: row.time,
                shot_start: row.shot_start,
                shot_end: row.shot_end,
                score,
            });
        }
    }

    // Sort by score descending (stable; preserves insertion order on ties).
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if let Some(min) = min_score {
        hits.retain(|h| h.score >= min);
    }

    let top = match hits.first() {
        Some(h) if h.score > 0.0 => h.score,
        _ => return Vec::new(),
    };
    let floor = top * relative_cutoff;

    // Limit FIRST, then floor — result is always ≤ limit (upstream order).
    hits.into_iter()
        .take(limit)
        .filter(|h| h.score >= floor)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::embed_store::{Header, Row};

    fn index(id_dim: usize, rows: Vec<Row>, vectors: Vec<f32>) -> AssetIndex {
        AssetIndex {
            header: Header {
                model: "m".into(),
                model_version: 1,
                sampler_version: 1,
                dim: id_dim,
                count: rows.len(),
            },
            rows,
            vectors,
        }
    }

    fn row(t: f64, shot: f64) -> Row {
        Row {
            time: t,
            shot_start: shot,
            shot_end: shot + 1.0,
        }
    }

    #[test]
    fn dot_product_scores_rank_by_similarity() {
        // dim 2; query (1,0). rows: (1,0)->1.0, (0.5,0)->0.5, (0,1)->0.0
        let idx = index(
            2,
            vec![row(0.0, 0.0), row(1.0, 1.0), row(2.0, 2.0)],
            vec![1.0, 0.0, 0.5, 0.0, 0.0, 1.0],
        );
        let hits = search(&[1.0, 0.0], &[("a".into(), idx)], 20, 0.0, None);
        assert_eq!(hits[0].time, 0.0); // score 1.0 first
        assert_eq!(hits[0].score, 1.0);
        assert!(hits[0].score >= hits[1].score);
    }

    #[test]
    fn best_per_shot_dedupes_same_shot() {
        // Two frames in shot 0 (scores 0.9, 0.3), one frame shot 5 (score 0.5).
        let idx = index(
            1,
            vec![row(0.0, 0.0), row(1.0, 0.0), row(5.0, 5.0)],
            vec![0.9, 0.3, 0.5],
        );
        let hits = search(&[1.0], &[("a".into(), idx)], 20, 0.0, None);
        // shot 0 collapses to its best (0.9); shot 5 stays (0.5) → 2 hits.
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].score, 0.9);
        assert_eq!(hits[0].time, 0.0);
        assert_eq!(hits[1].score, 0.5);
    }

    #[test]
    fn best_per_shot_ties_keep_first() {
        // Two frames in shot 0 with equal score; first (time 0) must win.
        let idx = index(1, vec![row(0.0, 0.0), row(9.0, 0.0)], vec![0.5, 0.5]);
        let hits = search(&[1.0], &[("a".into(), idx)], 20, 0.0, None);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].time, 0.0); // first occurrence wins the tie
    }

    #[test]
    fn min_score_filters_before_ranking() {
        let idx = index(
            1,
            vec![row(0.0, 0.0), row(1.0, 1.0), row(2.0, 2.0)],
            vec![0.9, 0.04, 0.5],
        );
        // min_score 0.05 drops the 0.04 frame.
        let hits = search(&[1.0], &[("a".into(), idx)], 20, 0.0, Some(0.05));
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| h.score >= 0.05));
    }

    #[test]
    fn relative_cutoff_filters_low_scores() {
        let idx = index(
            1,
            vec![row(0.0, 0.0), row(1.0, 1.0), row(2.0, 2.0)],
            vec![1.0, 0.9, 0.5],
        );
        // top=1.0, cutoff 0.85 → floor 0.85; 0.5 dropped, 0.9 kept.
        let hits = search(&[1.0], &[("a".into(), idx)], 20, 0.85, None);
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| h.score >= 0.85));
    }

    #[test]
    fn limit_applies_before_floor() {
        // 3 high scores all above floor, but limit=2 → exactly 2 even though a
        // 3rd would pass the floor.
        let idx = index(
            1,
            vec![row(0.0, 0.0), row(1.0, 1.0), row(2.0, 2.0)],
            vec![1.0, 0.95, 0.9],
        );
        let hits = search(&[1.0], &[("a".into(), idx)], 2, 0.85, None);
        assert_eq!(hits.len(), 2); // floor would keep all 3, but limit cuts to 2
        assert_eq!(hits[0].score, 1.0);
        assert_eq!(hits[1].score, 0.95);
    }

    #[test]
    fn empty_when_top_not_positive() {
        let idx = index(1, vec![row(0.0, 0.0)], vec![0.0]);
        let hits = search(&[1.0], &[("a".into(), idx)], 20, 0.85, None);
        assert!(hits.is_empty());

        // negative scores too
        let idx2 = index(1, vec![row(0.0, 0.0)], vec![-1.0]);
        let hits2 = search(&[1.0], &[("a".into(), idx2)], 20, 0.85, None);
        assert!(hits2.is_empty());
    }

    #[test]
    fn dim_mismatch_index_is_skipped() {
        let good = index(2, vec![row(0.0, 0.0)], vec![1.0, 0.0]);
        let bad = index(3, vec![row(0.0, 0.0)], vec![1.0, 0.0, 0.0]);
        let hits = search(
            &[1.0, 0.0],
            &[("bad".into(), bad), ("good".into(), good)],
            20,
            0.0,
            None,
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].asset_id, "good");
    }

    #[test]
    fn empty_count_index_skipped() {
        let empty = index(2, vec![], vec![]);
        let hits = search(&[1.0, 0.0], &[("e".into(), empty)], 20, 0.0, None);
        assert!(hits.is_empty());
    }

    #[test]
    fn multi_asset_results_merge_and_sort() {
        let a = index(1, vec![row(0.0, 0.0)], vec![0.7]);
        let b = index(1, vec![row(3.0, 3.0)], vec![0.9]);
        let hits = search(&[1.0], &[("a".into(), a), ("b".into(), b)], 20, 0.0, None);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].asset_id, "b"); // higher score first
        assert_eq!(hits[1].asset_id, "a");
    }
}
