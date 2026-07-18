use criterion::{criterion_group, criterion_main, Criterion};
use openlife_core::embedding::{EmbeddingProfile, EmbeddingRouteKind};
use openlife_core::vectors::{VectorInsertItem, VectorStore};
use std::hint::black_box;

fn benchmark_profile() -> EmbeddingProfile {
    EmbeddingProfile::new(
        EmbeddingRouteKind::DeterministicHash,
        "openlife-benchmark",
        "deterministic-benchmark-v1",
        "builtin:benchmark",
        "deterministic-benchmark-artifact-v1",
        384,
    )
    .unwrap()
}

fn bench_vector_search(c: &mut Criterion) {
    let store = VectorStore::new_in_memory().unwrap();
    let profile = benchmark_profile();

    // Prepare test data with owned strings
    let sessions: Vec<String> = (0..1000).map(|i| format!("session-{}", i % 10)).collect();
    let contents: Vec<String> = (0..1000).map(|i| format!("Test content {}", i)).collect();
    let embeddings: Vec<Vec<f32>> = (0..1000)
        .map(|i| {
            (0..384)
                .map(|j| ((i * 384 + j) % 100) as f32 / 100.0)
                .collect()
        })
        .collect();

    // Insert 1000 test chunks
    let items: Vec<VectorInsertItem> = (0..1000)
        .map(|i| VectorInsertItem {
            session_id: &sessions[i],
            content: &contents[i],
            embedding: &embeddings[i],
            profile: &profile,
            source: "benchmark",
        })
        .collect();
    store.insert_batch(&items).unwrap();

    let query: Vec<f32> = (0..384).map(|i| (i % 100) as f32 / 100.0).collect();

    c.bench_function("vector_search_1000_chunks", |b| {
        b.iter(|| {
            store
                .search(black_box(&query), black_box(&profile), black_box(5))
                .unwrap()
        })
    });
}

fn bench_vector_insert(c: &mut Criterion) {
    c.bench_function("vector_insert_single", |b| {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = benchmark_profile();
        let embedding: Vec<f32> = (0..384).map(|i| (i % 100) as f32 / 100.0).collect();
        let session_id = "session-1".to_string();
        let content = "test content".to_string();

        b.iter(|| {
            store
                .insert(
                    black_box(&session_id),
                    black_box(&content),
                    black_box(&embedding),
                    black_box(&profile),
                    black_box("benchmark"),
                )
                .unwrap()
        })
    });
}

criterion_group!(benches, bench_vector_search, bench_vector_insert);
criterion_main!(benches);
