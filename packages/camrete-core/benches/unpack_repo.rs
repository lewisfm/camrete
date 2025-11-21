use camrete_core::database::models::RepositoryRef;
use camrete_core::repo::client::{DirUnpacker, DownloadProgressReporter, RepoManager};
use criterion::{Criterion, criterion_main};
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

                let repo_path = PathBuf::from("./benches/mini_repo");
                let progress = Arc::new(DownloadProgressReporter::new(None, Box::new(|_| {})));

                let url = Url::parse("about:blank").unwrap();
                let repo_ref = RepositoryRef::shared("benchmark", &url);
                let repo = repo_mgr.db().unwrap().create_empty_repo(repo_ref).unwrap();

                for _i in 0..iters {
                    let unpacker = DirUnpacker::new(repo_path.clone()).await.unwrap();

                    let start = Instant::now();
                    repo_mgr
                        .unpack_repo(&repo, unpacker, None, progress.clone())
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
