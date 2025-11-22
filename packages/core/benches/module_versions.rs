use camrete_core::database::models::module::ModuleVersion;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use rand::{Rng, SeedableRng, distr::Uniform, rngs::SmallRng, seq::IndexedRandom};

fn random_version(rng: &mut impl Rng) -> String {
    let num_parts = rng.random_range(1..=4);
    let mut version = String::new();

    if rng.random_bool(0.1) {
        version.push('v');
    }

    let allowed_subversions = Uniform::new_inclusive(0, 15).unwrap();

    version += &(0..num_parts)
        .map(|_i| {
            let subv = rng.sample(allowed_subversions);

            if rng.random_bool(0.9) {
                return format!("{subv}");
            }

            let mut tag = ["alpha", "beta", "pre"].choose(rng).unwrap().to_string();

            if rng.random_bool(0.25) {
                tag.truncate(1);
            }

            if rng.random_bool(0.5) {
                tag += &format!("{}", rng.sample(allowed_subversions));
            }

            tag
        })
        .collect::<Vec<_>>()
        .join(".");

    version
}

fn bench(c: &mut Criterion) {
    c.bench_function("cmp_rand_versions", |b| {
        let mut rng = SmallRng::seed_from_u64(8096041311318838857);

        b.iter_batched_ref(
            move || (random_version(&mut rng), random_version(&mut rng)),
            |(left, right)| {
                let left = ModuleVersion::from(left.as_str());
                let right = ModuleVersion::from(right.as_str());

                left.cmp(&right)
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
