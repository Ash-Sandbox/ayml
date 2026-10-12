//! AYML vs `serde_json` benchmarks. Command: `cargo bench --bench serde_comparison`

use std::fmt::Write as _;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use serde::{Deserialize, Serialize};

// ── Shared types ────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct FlatConfig {
    host: String,
    port: u16,
    debug: bool,
    workers: u32,
    name: String,
    version: String,
}

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

#[derive(Serialize, Deserialize)]
struct Person {
    name: String,
    age: u32,
    email: String,
    active: bool,
}

#[derive(Serialize, Deserialize)]
struct Article {
    title: String,
    description: String,
    author: String,
    tags: Vec<String>,
    notes: String,
    url: String,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    id: u32,
    name: String,
    email: String,
    score: f64,
    active: bool,
}

// ── Inputs ──────────────────────────────────────────────────────────

const FLAT_AYML: &str = "\
host: localhost
port: 8080
debug: false
workers: 4
name: my-application
version: 1.0.0
";

const FLAT_JSON: &str = r#"{"host":"localhost","port":8080,"debug":false,"workers":4,"name":"my-application","version":"1.0.0"}"#;

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

const NESTED_JSON: &str = r#"{"server":{"host":"localhost","port":8080},"database":{"host":"db.example.com","port":5432,"name":"mydb","pool_size":10},"logging":{"level":"info","file":"/var/log/app.log"}}"#;

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

const SEQ_OF_MAPS_JSON: &str = r#"[{"name":"Alice","age":30,"email":"alice@example.com","active":true},{"name":"Bob","age":25,"email":"bob@example.com","active":false},{"name":"Charlie","age":35,"email":"charlie@example.com","active":true},{"name":"Diana","age":28,"email":"diana@example.com","active":true}]"#;

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

const STRINGS_JSON: &str = r#"{"title":"The Quick Brown Fox Jumps Over the Lazy Dog","description":"A pangram is a sentence using every letter of the alphabet at least once.","author":"Anonymous Author","tags":["pangram","linguistics","typography","language arts","character sets"],"notes":"This sentence has been used since at least the late 19th century.","url":"https://en.wikipedia.org/wiki/The_quick_brown_fox_jumps_over_the_lazy_dog"}"#;

fn make_large_ayml() -> String {
    let mut s = String::new();
    for i in 0..50 {
        let _ = write!(
            s,
            "- id: {i}\n  name: user-{i}\n  email: user-{i}@example.com\n  score: {}.{}\n  active: {}\n",
            i * 17 % 100,
            i * 31 % 100,
            i % 2 == 0,
        );
    }
    s
}

fn make_large_json() -> String {
    let entries: Vec<Entry> = ayml::from_str(&make_large_ayml()).unwrap();
    serde_json::to_string(&entries).unwrap()
}

// ── Benchmarks ──────────────────────────────────────────────────────

macro_rules! bench_de_pair {
    ($group:expr, $name:expr, $ayml:expr, $json:expr, $type:ty) => {
        $group.throughput(Throughput::Bytes($ayml.len() as u64));
        $group.bench_with_input(BenchmarkId::new("ayml", $name), $ayml, |b, s| {
            b.iter(|| ayml::from_str::<$type>(s).unwrap());
        });
        $group.throughput(Throughput::Bytes($json.len() as u64));
        $group.bench_with_input(BenchmarkId::new("json", $name), $json, |b, s| {
            b.iter(|| serde_json::from_str::<$type>(s).unwrap());
        });
    };
}

fn bench_deserialize(c: &mut Criterion) {
    let large_ayml = make_large_ayml();
    let large_json = make_large_json();

    let mut group = c.benchmark_group("deserialize");

    bench_de_pair!(group, "flat", FLAT_AYML, FLAT_JSON, FlatConfig);
    bench_de_pair!(group, "nested", NESTED_AYML, NESTED_JSON, NestedConfig);
    bench_de_pair!(
        group,
        "seq_of_maps",
        SEQ_OF_MAPS_AYML,
        SEQ_OF_MAPS_JSON,
        Vec<Person>
    );
    bench_de_pair!(group, "strings", STRINGS_AYML, STRINGS_JSON, Article);
    bench_de_pair!(group, "large_50", &large_ayml, &large_json, Vec<Entry>);

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

    macro_rules! bench_ser_pair {
        ($name:expr, $value:expr) => {
            let ayml_size = ayml::to_string(&$value).unwrap().len();
            group.throughput(Throughput::Bytes(ayml_size as u64));
            group.bench_function(BenchmarkId::new("ayml", $name), |b| {
                b.iter(|| ayml::to_string(&$value).unwrap());
            });

            let json_size = serde_json::to_string(&$value).unwrap().len();
            group.throughput(Throughput::Bytes(json_size as u64));
            group.bench_function(BenchmarkId::new("json", $name), |b| {
                b.iter(|| serde_json::to_string(&$value).unwrap());
            });
        };
    }

    bench_ser_pair!("flat", flat);
    bench_ser_pair!("nested", nested);
    bench_ser_pair!("seq_of_maps", seq_of_maps);
    bench_ser_pair!("strings", strings);
    bench_ser_pair!("large_50", large);

    group.finish();
}

criterion_group!(benches, bench_deserialize, bench_serialize);
criterion_main!(benches);
