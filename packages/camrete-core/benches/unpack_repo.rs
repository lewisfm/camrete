use camrete_core::database::models::RepositoryRef;
use camrete_core::repo::{RepoManager, TarGzAssetLoader};
use camrete_core::repo::asset_stream::bench::{AssetDirLoader, InMemoryAssetLoader};
use camrete_core::repo::client::DownloadProgressReporter;
use criterion::{Criterion, criterion_main};
use tokio::fs::read;
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use url::Url;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("unpack_repo", |b| {
        b.to_async(Runtime::new().unwrap())
            .iter_custom(|iters| async move {
                let mut total = Duration::ZERO;
                let mut repo_mgr = RepoManager::new("../../target/bench.db").unwrap();

                let repo_data = read("./benches/mini_repo.tgz").await.unwrap();
                let progress = Arc::new(DownloadProgressReporter::new(None, Box::new(|_| {})));

                let url = Url::parse("about:blank").unwrap();
                let repo_ref = RepositoryRef::shared("benchmark", &url);
                let repo = repo_mgr.db().unwrap().create_empty_repo(repo_ref).unwrap();

                let loader = TarGzAssetLoader::from_buf(repo_data);
                let repo_assets = InMemoryAssetLoader::from_loader(loader).await.unwrap();
                assert!(!repo_assets.assets.is_empty());

                for _i in 0..iters {
                    let assets = repo_assets.clone();

                    let start = Instant::now();
                    repo_mgr
                        .unpack_repo(
                            black_box(&repo),
                            black_box(assets),
                            black_box(None),
                            black_box(progress.clone()),
                        )
                        .await
                        .unwrap();
                    total += start.elapsed()
                }

                total
            });
    });
}

pub fn benches() {
    let mut criterion: Criterion<_> = Criterion::default().configure_from_args();

    criterion_benchmark(&mut criterion);
}

criterion_main!(benches);
