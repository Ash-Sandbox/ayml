/// Tests based on the full-length examples from the AYML spec.
use ayml_core::{
    parse,
    value::{MapKey, Value},
};

#[test]
fn spec_invoice() {
    let input = "\
invoice: 34843
date: 2001-01-23
bill-to:
  given: Chris
  family: Dumars
  address:
    lines: \"\"\"
      458 Walkman Dr.
      Suite #292
      \"\"\"
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
comments: \"\"\"
  Late afternoon is best.
  Backup contact is Nancy
  Billsmer @ 338-4338.
  \"\"\"";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();

    assert_eq!(
        map[&MapKey::String("invoice".into())].value,
        Value::Int(34843)
    );
    assert_eq!(
        map[&MapKey::String("date".into())].value,
        Value::Str("2001-01-23".into())
    );
    assert!(map[&MapKey::String("ship-to".into())].value.is_null());
    assert_eq!(
        map[&MapKey::String("tax".into())].value,
        Value::Float(251.42)
    );
    assert_eq!(
        map[&MapKey::String("total".into())].value,
        Value::Float(4443.42)
    );

    // bill-to
    let bill_to = map[&MapKey::String("bill-to".into())]
        .value
        .as_mapping()
        .unwrap();
    assert_eq!(
        bill_to[&MapKey::String("given".into())].value,
        Value::Str("Chris".into())
    );

    // address
    let address = bill_to[&MapKey::String("address".into())]
        .value
        .as_mapping()
        .unwrap();
    let lines = address[&MapKey::String("lines".into())]
        .value
        .as_str()
        .unwrap();
    assert!(lines.contains("458 Walkman Dr."));
    assert!(lines.contains("Suite #292"));
    assert_eq!(
        address[&MapKey::String("city".into())].value,
        Value::Str("Royal Oak".into())
    );
    assert_eq!(
        address[&MapKey::String("postal".into())].value,
        Value::Int(48046)
    );

    // products
    let products = map[&MapKey::String("product".into())]
        .value
        .as_sequence()
        .unwrap();
    assert_eq!(products.len(), 2);
    let p0 = products[0].value.as_mapping().unwrap();
    assert_eq!(
        p0[&MapKey::String("sku".into())].value,
        Value::Str("BL394D".into())
    );
    assert_eq!(p0[&MapKey::String("quantity".into())].value, Value::Int(4));
    assert_eq!(
        p0[&MapKey::String("price".into())].value,
        Value::Float(450.0)
    );

    // comments
    let comments = map[&MapKey::String("comments".into())]
        .value
        .as_str()
        .unwrap();
    assert!(comments.contains("Late afternoon is best."));
}

#[test]
fn spec_log_file() {
    let input = "\
Date: 2001-11-23T15:03:17-5:00
User: ed
Fatal: Unknown variable \"bar\"
Stack:
- file: TopClass.py
  line: 23
  code: \"\"\"
    x = MoreObject(\"345\\n\")
    \"\"\"
- file: MoreClass.py
  line: 58
  code: \"\"\"
    foo = bar
    \"\"\"";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();

    assert_eq!(
        map[&MapKey::String("User".into())].value,
        Value::Str("ed".into())
    );

    let stack = map[&MapKey::String("Stack".into())]
        .value
        .as_sequence()
        .unwrap();
    assert_eq!(stack.len(), 2);

    let s0 = stack[0].value.as_mapping().unwrap();
    assert_eq!(
        s0[&MapKey::String("file".into())].value,
        Value::Str("TopClass.py".into())
    );
    assert_eq!(s0[&MapKey::String("line".into())].value, Value::Int(23));

    let code0 = s0[&MapKey::String("code".into())].value.as_str().unwrap();
    assert!(code0.contains("MoreObject"));
}

#[test]
fn spec_player_statistics() {
    let input = "\
hr:  65    # Home runs
avg: 0.278 # Batting average
rbi: 147   # Runs Batted In";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();

    assert_eq!(map[&MapKey::String("hr".into())].value, Value::Int(65));
    assert_eq!(
        map[&MapKey::String("avg".into())].value,
        Value::Float(0.278)
    );
    assert_eq!(map[&MapKey::String("rbi".into())].value, Value::Int(147));

    // Check inline comments preserved
    assert_eq!(
        map[&MapKey::String("hr".into())].inline_comment.as_deref(),
        Some("Home runs")
    );
}

#[test]
fn spec_mapping_of_flow_mappings() {
    let input = "\
Mark McGwire: {hr: 65, avg: 0.278}
Sammy Sosa: {
  hr: 63,
  avg: 0.288,
}";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();

    let mcgwire = map[&MapKey::String("Mark McGwire".into())]
        .value
        .as_mapping()
        .unwrap();
    assert_eq!(mcgwire[&MapKey::String("hr".into())].value, Value::Int(65));

    let sosa = map[&MapKey::String("Sammy Sosa".into())]
        .value
        .as_mapping()
        .unwrap();
    assert_eq!(sosa[&MapKey::String("hr".into())].value, Value::Int(63));
}

#[test]
fn spec_two_comments() {
    let input = "\
hr: # 1998 hr ranking
- Mark McGwire
- Sammy Sosa
# 1998 rbi ranking
rbi:
- Sammy Sosa
- Ken Griffey";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();

    let hr = map[&MapKey::String("hr".into())]
        .value
        .as_sequence()
        .unwrap();
    assert_eq!(hr.len(), 2);
    assert_eq!(hr[0].value, Value::Str("Mark McGwire".into()));

    let rbi = map[&MapKey::String("rbi".into())]
        .value
        .as_sequence()
        .unwrap();
    assert_eq!(rbi.len(), 2);
    assert_eq!(rbi[0].value, Value::Str("Sammy Sosa".into()));
}

#[test]
fn spec_network_with_comments() {
    let input = "\
# Network connection rules
# Use these rules to allow socket connections
network:
  rules:
    - host: github.com
      ports:
      - 22 # Git (SSH)
      - 443 # Site";
    let node = parse(input).unwrap();

    // Top comment
    assert!(node.comment.is_some());

    let map = node.value.as_mapping().unwrap();
    let network = map[&MapKey::String("network".into())]
        .value
        .as_mapping()
        .unwrap();
    let rules = network[&MapKey::String("rules".into())]
        .value
        .as_sequence()
        .unwrap();
    assert_eq!(rules.len(), 1);

    let rule0 = rules[0].value.as_mapping().unwrap();
    assert_eq!(
        rule0[&MapKey::String("host".into())].value,
        Value::Str("github.com".into())
    );

    let ports = rule0[&MapKey::String("ports".into())]
        .value
        .as_sequence()
        .unwrap();
    assert_eq!(ports.len(), 2);
    assert_eq!(ports[0].value, Value::Int(22));
    assert_eq!(ports[0].inline_comment.as_deref(), Some("Git (SSH)"));
    assert_eq!(ports[1].value, Value::Int(443));
    assert_eq!(ports[1].inline_comment.as_deref(), Some("Site"));
}
