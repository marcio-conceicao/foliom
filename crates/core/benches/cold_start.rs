//! ACPT-02 cold-start benchmark (plan 02-08, D-35).
//!
//! Measures `Db::open_at + reindex(Full)` over a synthetic 5k corpus.
//! Each iteration uses a fresh `tempfile::tempdir()` to force a true
//! cold start (no shared page cache, fresh `index.db` file).
//!
//! Corpus path defaults to `/tmp/synth-5k`. Override with
//! `FOLIOM_BENCH_CORPUS=/path` (so non-Linux dev environments can
//! point at a project-local directory).
//!
//! Generate the corpus first:
//!     cargo run --release --bin foliom-bench-gen -- \
//!         --out /tmp/synth-5k --count 5000 --seed 42
//!
//! Run the bench:
//!     cargo bench --bench cold_start
//!
//! Gate the CI run:
//!     python3 scripts/bench_assert.py \
//!         target/criterion/cold_start_5k/*/new/estimates.json \
//!         3000000000

use std::path::PathBuf;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use foliom_core::indexer::{reindex, ReindexMode};
use foliom_core::storage::Db;

fn corpus_path() -> PathBuf {
    std::env::var_os("FOLIOM_BENCH_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/synth-5k"))
}

fn cold_start(c: &mut Criterion) {
    let corpus = corpus_path();
    if !corpus.exists() {
        eprintln!("skip cold_start_5k — corpus missing at {}. \
                   Generate with `cargo run --release --bin \
                   foliom-bench-gen -- --out {} --count 5000`.",
                  corpus.display(), corpus.display());
        return;
    }

    let mut group = c.benchmark_group("cold_start_5k");
    // 5k-file reindex is slow; cap measurement time and sample count so
    // the bench finishes in a few minutes on CI runners.
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(10);

    // Bench function name avoids `:` / `::` because Criterion sanitizes
    // them in the output directory (e.g. `Db::open` → `Db__open`). A
    // plain underscore-separated name keeps the path predictable for
    // `scripts/bench_assert.py` and the CI glob.
    group.bench_function("db_open_reindex_full", |b| {
        b.iter_with_setup(
            || {
                // Fresh DB path each iteration — forces cold-start path
                // (open + migrate + reindex), bypassing OS file cache as
                // best we can without dropping kernel caches.
                let tmp = tempfile::tempdir().expect("tempdir");
                let db_path = tmp.path().join("index.db");
                (tmp, db_path)
            },
            |(tmp, db_path)| {
                let mut db = Db::open_at(&db_path).expect("Db::open_at");
                let _stats = reindex(&mut db, &corpus, ReindexMode::Full)
                    .expect("reindex Full");
                drop(tmp);
            },
        )
    });
    group.finish();
}

criterion_group!(benches, cold_start);
criterion_main!(benches);
