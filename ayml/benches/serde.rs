use ayml::value::Value;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use serde::{Deserialize, Serialize};

// ── Workloads ───────────────────────────────────────────────────────

const FLAT_AYML: &str = "\
host: localhost
port: 8080
debug: false
workers: 4
name: my-application
version: 1.0.0
";

#[derive(Serialize, Deserialize)]
struct FlatConfig {
    host: String,
    port: u16,
    debug: bool,
    workers: u32,
    name: String,
    version: String,
}

const NESTED_AYML: &str = "\
server:
  host: localhost
  port: 8080
database:
  host: db.example.com
  port: 5432
  name: mydb
  pool_size: 10
logging:
  level: info
  file: /var/log/app.log
";

#[derive(Serialize, Deserialize)]
struct NestedConfig {
    server: Server,
    database: Database,
    logging: Logging,
}

#[derive(Serialize, Deserialize)]
struct Server {
    host: String,
    port: u16,
}

#[derive(Serialize, Deserialize)]
struct Database {
    host: String,
    port: u16,
    name: String,
    pool_size: u32,
}

#[derive(Serialize, Deserialize)]
struct Logging {
    level: String,
    file: String,
}

const SEQ_OF_MAPS_AYML: &str = "\
- name: Alice
  age: 30
  email: alice@example.com
  active: true
- name: Bob
  age: 25
  email: bob@example.com
  active: false
- name: Charlie
  age: 35
  email: charlie@example.com
  active: true
- name: Diana
  age: 28
  email: diana@example.com
  active: true
";

#[derive(Serialize, Deserialize)]
struct Person {
    name: String,
    age: u32,
    email: String,
    active: bool,
}

const STRINGS_AYML: &str = "\
title: \"The Quick Brown Fox Jumps Over the Lazy Dog\"
description: \"A pangram is a sentence using every letter of the alphabet at least once.\"
author: \"Anonymous Author\"
tags:
  - pangram
  - linguistics
  - typography
  - \"language arts\"
  - \"character sets\"
notes: \"This sentence has been used since at least the late 19th century.\"
url: \"https://en.wikipedia.org/wiki/The_quick_brown_fox_jumps_over_the_lazy_dog\"
";

#[derive(Serialize, Deserialize)]
struct Article {
    title: String,
    description: String,
    author: String,
    tags: Vec<String>,
    notes: String,
    url: String,
}

fn make_large_ayml() -> String {
    let mut s = String::new();
    for i in 0..50 {
        s.push_str(&format!(
            "- id: {i}\n  name: user-{i}\n  email: user-{i}@example.com\n  score: {}.{}\n  active: {}\n",
            i * 17 % 100,
            i * 31 % 100,
            i % 2 == 0,
        ));
    }
    s
}

#[derive(Serialize, Deserialize)]
struct Entry {
    id: u32,
    name: String,
    email: String,
    score: f64,
    active: bool,
}

// ── Benchmarks ──────────────────────────────────────────────────────

macro_rules! bench_de {
    ($group:expr, $name:expr, $input:expr, $type:ty) => {
        $group.throughput(Throughput::Bytes($input.len() as u64));
        $group.bench_with_input(BenchmarkId::new("typed", $name), $input, |b, s| {
            b.iter(|| ayml::from_str::<$type>(s).unwrap());
        });
        $group.bench_with_input(BenchmarkId::new("value", $name), $input, |b, s| {
            b.iter(|| ayml::from_str::<Value>(s).unwrap());
        });
    };
}

fn bench_deserialize(c: &mut Criterion) {
    let large_ayml = make_large_ayml();
    let mut group = c.benchmark_group("deserialize");

    bench_de!(group, "flat", FLAT_AYML, FlatConfig);
    bench_de!(group, "nested", NESTED_AYML, NestedConfig);
    bench_de!(group, "seq_of_maps", SEQ_OF_MAPS_AYML, Vec<Person>);
    bench_de!(group, "strings", STRINGS_AYML, Article);
    bench_de!(group, "large_50", &large_ayml, Vec<Entry>);

    group.finish();
}

fn bench_serialize(c: &mut Criterion) {
    let flat: FlatConfig = ayml::from_str(FLAT_AYML).unwrap();
    let nested: NestedConfig = ayml::from_str(NESTED_AYML).unwrap();
    let seq_of_maps: Vec<Person> = ayml::from_str(SEQ_OF_MAPS_AYML).unwrap();
    let strings: Article = ayml::from_str(STRINGS_AYML).unwrap();
    let large_ayml = make_large_ayml();
    let large: Vec<Entry> = ayml::from_str(&large_ayml).unwrap();

    let mut group = c.benchmark_group("serialize");

    macro_rules! bench_ser {
        ($name:expr, $value:expr) => {
            let size = ayml::to_string(&$value).unwrap().len();
            group.throughput(Throughput::Bytes(size as u64));
            group.bench_function($name, |b| {
                b.iter(|| ayml::to_string(&$value).unwrap());
            });
        };
    }

    bench_ser!("flat", flat);
    bench_ser!("nested", nested);
    bench_ser!("seq_of_maps", seq_of_maps);
    bench_ser!("strings", strings);
    bench_ser!("large_50", large);

    group.finish();
}

criterion_group!(benches, bench_deserialize, bench_serialize);
criterion_main!(benches);
