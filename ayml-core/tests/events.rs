//! Event streams of `Reader` and `NodeReader`.

use ayml_core::reader::Event;
use ayml_core::{NodeReader, Reader, parse};

/// Return all events of a document, or the first error.
fn read_all(input: &str) -> Result<Vec<Event<'_>>, ayml_core::Error> {
    let mut reader = Reader::new(input);
    let mut events = Vec::new();
    loop {
        let event = reader.next_event()?;
        let done = matches!(event, Event::DocumentEnd { .. });
        events.push(event);
        if done {
            return Ok(events);
        }
    }
}

#[test]
fn errors_are_final() {
    for input in [
        "{a: 1, a: 2}\n",
        "a: [1, 2\nb: 3\n",
        "- a\n-b\n- c\n",
        "a: 1\n  b: 2\n",
    ] {
        let mut reader = Reader::new(input);
        let error = loop {
            match reader.next_event() {
                Ok(Event::DocumentEnd { .. }) => panic!("{input:?} is invalid"),
                Ok(_) => {}
                Err(e) => break e,
            }
        };
        for _ in 0..3 {
            assert_eq!(reader.next_event(), Err(error.clone()), "{input:?}");
        }
    }
}

#[test]
fn document_end_repeats() {
    let mut reader = Reader::new("a: 1\n# done\n");
    while !matches!(reader.next_event().unwrap(), Event::DocumentEnd { .. }) {}
    assert_eq!(
        reader.next_event().unwrap(),
        Event::DocumentEnd { trailing: None }
    );
}

#[test]
fn node_reader_replays_reader() {
    for input in [
        "# top\na: 1 # one\nb:\n  - x\n  # end of b\n# end\n",
        "- - a\n  - b\n- {k: v, # inline\n  # lead\n  w: [1, 2]}\n",
        "\"quoted \\n key\": \"\"\"\n  text\n  \"\"\"\n",
        "plain scalar # c\n# trailing\n",
        "[1, 2] # c\n",
    ] {
        let tree = parse(input).unwrap();
        let mut nodes = NodeReader::new(&tree);
        let from_text = read_all(input).unwrap();
        let from_tree: Vec<_> = from_text.iter().map(|_| nodes.next_event()).collect();
        assert_eq!(from_tree, from_text, "{input:?}");
    }
}
