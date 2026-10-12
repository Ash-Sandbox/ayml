//! Round trips: serialize then deserialize, and parse then re-serialize.

#![allow(clippy::items_after_statements)]

use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;

use ayml::Value;
use serde::{Deserialize, Serialize};

#[track_caller]
fn roundtrip<T: Serialize + for<'de> Deserialize<'de> + Debug + PartialEq>(value: &T) {
    let text = ayml::to_string(value).unwrap();
    let back: T = ayml::from_str(&text).unwrap_or_else(|e| panic!("{e}\n--- text ---\n{text}"));
    assert_eq!(&back, value, "\n--- text ---\n{text}");
}

#[test]
fn scalars() {
    roundtrip(&());
    roundtrip(&true);
    for v in [i64::MIN, -1, 0, 1, i64::MAX] {
        roundtrip(&v);
    }
    roundtrip(&i8::MIN);
    roundtrip(&u16::MAX);
    roundtrip(&u32::MAX);
    roundtrip(&(i64::MAX as u64));
    roundtrip(&i128::from(i64::MIN));
    for v in [
        0.0,
        -0.0,
        0.1,
        1.0,
        1e300,
        5e-324,
        f64::MAX,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        roundtrip(&v);
    }
    for v in [0.1f32, f32::MAX, f32::MIN_POSITIVE, 1.0e-45] {
        roundtrip(&v);
    }
    for c in ['a', '"', '\n', '\0', '\u{FEFF}', '😀'] {
        roundtrip(&c);
    }
}

#[test]
fn strings() {
    for s in [
        "",
        "null",
        "true",
        "inf",
        "-inf",
        "nan",
        "42",
        "-7",
        "3.25",
        "0x10",
        "1e5",
        "007",
        "yes",
        "- item",
        "-",
        ":",
        "a: b",
        "a #b",
        "a#b",
        "[x]",
        "{x}",
        "'x'",
        "\"x\"",
        " x",
        "x ",
        "\t",
        "\u{A0}",
        "\u{FEFF}x",
        "\u{2028}",
        "\u{FFFE}",
        "\u{7F}",
        "a\nb",
        "a\n",
        "\n",
        "\n\n",
        "a\r\nb",
        "a\n\"\"\"\nb",
        "a\n\\n",
        "  indented\n    more",
        "trailing \nspace\t",
    ] {
        roundtrip(&s.to_string());
    }
}

#[test]
fn options_and_collections() {
    roundtrip(&None::<i32>);
    roundtrip(&Some("x".to_string()));
    roundtrip(&vec![Some(1), None, Some(3)]);
    roundtrip(&Vec::<i32>::new());
    roundtrip(&vec![vec![1, 2], vec![], vec![3]]);
    roundtrip(&vec![vec![vec!["deep".to_string()]]]);
    roundtrip(&(1, "two".to_string(), 3.0));
    roundtrip(&BTreeMap::<String, i32>::new());
    roundtrip(&BTreeMap::from([
        (String::new(), 1),
        ("true".to_string(), 2),
        ("a: b".to_string(), 3),
    ]));
    roundtrip(&BTreeMap::from([
        (1u32, "a".to_string()),
        (2, "b".to_string()),
    ]));
    roundtrip(&BTreeMap::from([(-5i64, vec![1]), (7, vec![])]));
    roundtrip(&HashMap::from([(true, 1), (false, 2)]));
}

#[test]
fn structs_and_enums() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Unit;
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Newtype(String);
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    enum E {
        A,
        B(i32),
        C(i32, String),
        D { x: Option<f64>, y: Vec<E> },
    }
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct S {
        unit: Unit,
        newtype: Newtype,
        enums: Vec<E>,
        nested: Option<Box<S>>,
    }
    roundtrip(&S {
        unit: Unit,
        newtype: Newtype("multi\nline".into()),
        enums: vec![
            E::A,
            E::B(-1),
            E::C(2, "x".into()),
            E::D {
                x: None,
                y: vec![
                    E::A,
                    E::D {
                        x: Some(1.5),
                        y: vec![],
                    },
                ],
            },
        ],
        nested: Some(Box::new(S {
            unit: Unit,
            newtype: Newtype(String::new()),
            enums: vec![],
            nested: None,
        })),
    });
}

#[test]
fn spec_invoice() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    #[allow(clippy::struct_field_names)]
    struct Invoice {
        invoice: u32,
        date: String,
        #[serde(rename = "bill-to")]
        bill_to: Person,
        #[serde(rename = "ship-to")]
        ship_to: Option<Person>,
        product: Vec<Product>,
        tax: f64,
        total: f64,
        comments: String,
    }
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Person {
        given: String,
        family: String,
        address: Address,
    }
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Address {
        lines: String,
        city: String,
        state: String,
        postal: u32,
    }
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Product {
        sku: String,
        quantity: u32,
        description: String,
        price: f64,
    }
    let input = r#"invoice: 34843
date: 2001-01-23
bill-to:
  given: Chris
  family: Dumars
  address:
    lines: """
      458 Walkman Dr.
      Suite #292
      """
    city    : Royal Oak
    state   : MI
    postal  : 48046
ship-to: null
product:
  - sku         : BL394D
    quantity    : 4
    description : Basketball
    price       : 450.00
  - sku         : BL4438H
    quantity    : 1
    description : Super Hoop
    price       : 2392.00
tax  : 251.42
total: 4443.42
comments: """
  Late afternoon is best.
  Backup contact is Nancy
  Billsmer @ 338-4338.
  """
"#;
    let invoice: Invoice = ayml::from_str(input).unwrap();
    assert_eq!(invoice.bill_to.address.lines, "458 Walkman Dr.\nSuite #292");
    assert_eq!(invoice.bill_to.address.postal, 48046);
    assert!((invoice.product[1].price - 2392.0).abs() < f64::EPSILON);
    assert!(invoice.ship_to.is_none());
    assert!(invoice.comments.ends_with("338-4338."));
    roundtrip(&invoice);
}

#[test]
fn spec_log_file() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    #[serde(rename_all = "PascalCase")]
    struct Log {
        date: String,
        user: String,
        fatal: String,
        stack: Vec<Frame>,
    }
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Frame {
        file: String,
        line: u32,
        code: String,
    }
    let input = r#"Date: 2001-11-23T15:03:17-5:00
User: ed
Fatal: Unknown variable "bar"
Stack:
  - file: TopClass.py
    line: 23
    code: """
      x = MoreObject("345\n")
      """
  - file: MoreClass.py
    line: 58
    code: """
      foo = bar
      """
"#;
    let log: Log = ayml::from_str(input).unwrap();
    assert_eq!(log.date, "2001-11-23T15:03:17-5:00");
    assert_eq!(log.fatal, "Unknown variable \"bar\"");
    assert_eq!(log.stack[0].code, "x = MoreObject(\"345\n\")");
    roundtrip(&log);
}

#[test]
fn value_reserialization_is_canonical() {
    let input = "\
key:    value
list: [1,  2,
  3]
nested: {a: {b: \"c\"}}
seq:
- x
- - y
";
    let v: Value = ayml::from_str(input).unwrap();
    let text = ayml::to_string(&v).unwrap();
    assert_eq!(
        text,
        "key: value\nlist:\n  - 1\n  - 2\n  - 3\nnested:\n  a:\n    b: c\nseq:\n  - x\n  - - y\n"
    );
    assert_eq!(ayml::from_str::<Value>(&text).unwrap(), v);
}

#[test]
fn regression_keys() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Reserved {
        #[serde(rename = "true")]
        t: i32,
        #[serde(rename = "null")]
        n: i32,
        #[serde(rename = "a: b")]
        colon: i32,
    }
    roundtrip(&Reserved {
        t: 1,
        n: 2,
        colon: 3,
    });
    assert_eq!(
        ayml::to_string(&Reserved {
            t: 1,
            n: 2,
            colon: 3
        })
        .unwrap(),
        "true: 1\nnull: 2\n\"a: b\": 3\n"
    );
    roundtrip(&BTreeMap::from([
        ("line\nbreak".to_string(), 1),
        ("tab\t".to_string(), 2),
    ]));
    assert!(ayml::from_str::<Value>("a: 1\n\"a\": 2").is_err());
    let v: BTreeMap<String, i32> = ayml::from_str("1: 1\n0x1: 2").unwrap();
    assert_eq!(v.len(), 2);
}

#[test]
fn regression_triple_quoted_control_characters() {
    roundtrip(&"line\0\nnext\x01".to_string());
    roundtrip(&"\u{85}\n\u{2029}".to_string());
}
