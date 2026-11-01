use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Deserialize;
use serde::de::{Deserializer as _, Error as _, MapAccess, SeqAccess, Visitor};
use serde_json::Value;

use crate::read::{Batch, Batcher, Counted, Record, Unit};
use crate::{Error, Result};

const SAMPLE: usize = 1000;
const STOP: &str = "nd-transfer: stop";
const SHAPE: &str = "nd-transfer: shape";

struct Ordered(Vec<(String, Value)>);

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Ordered;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str(SHAPE)
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Ordered, A::Error> {
                let mut pairs = Vec::new();
                while let Some(pair) = map.next_entry::<String, Value>()? {
                    pairs.push(pair);
                }
                Ok(Ordered(pairs))
            }
        }
        d.deserialize_map(V)
    }
}

pub fn cell(v: Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        v @ (Value::Array(_) | Value::Object(_)) => Some(v.to_string()),
    }
}

struct Rows<'s, 'a> {
    out: &'s mut Batcher<'a>,
    keys: Vec<String>,
    pending: Vec<(u64, Ordered)>,
    fixed: bool,
    seen: Arc<AtomicU64>,
}

impl Rows<'_, '_> {
    fn record(keys: &[String], line: u64, obj: Ordered) -> Record {
        let mut values = vec![None; keys.len()];
        for (k, v) in obj.0 {
            if let Some(i) = keys.iter().position(|x| *x == k) {
                values[i] = cell(v);
            }
        }
        Record { line, values }
    }

    fn fix(&mut self) -> bool {
        if self.fixed {
            return true;
        }
        self.fixed = true;
        if !self.out.columns(self.keys.clone(), Vec::new()) {
            return false;
        }
        let done = self.seen.load(Ordering::Relaxed);
        for (line, obj) in std::mem::take(&mut self.pending) {
            if !self.out.push(Self::record(&self.keys, line, obj), || done) {
                return false;
            }
        }
        true
    }

    fn take(&mut self, line: u64, obj: Ordered) -> bool {
        if self.fixed {
            let seen = &self.seen;
            return self.out.push(Self::record(&self.keys, line, obj), || {
                seen.load(Ordering::Relaxed)
            });
        }
        for (k, _) in &obj.0 {
            if !self.keys.contains(k) {
                self.keys.push(k.clone());
            }
        }
        self.pending.push((line, obj));
        self.pending.len() < SAMPLE || self.fix()
    }

    fn end(mut self, total: u64) -> Result<()> {
        if self.keys.is_empty() {
            return Err(Error::NoColumns);
        }
        if !self.fix() {
            return Ok(());
        }
        self.out.flush(total);
        Ok(())
    }
}

struct Each<'r, 's, 'a>(&'r mut Rows<'s, 'a>);

impl<'de> Visitor<'de> for Each<'_, '_, '_> {
    type Value = ();

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(SHAPE)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<(), A::Error> {
        let mut index = 0u64;
        while let Some(obj) = seq.next_element::<Ordered>()? {
            index += 1;
            if !self.0.take(index, obj) {
                return Err(A::Error::custom(STOP));
            }
        }
        Ok(())
    }
}

fn fault(e: &serde_json::Error) -> Option<Error> {
    let text = e.to_string();
    if text.starts_with(STOP) {
        None
    } else if text.contains(SHAPE) {
        Some(Error::JsonShape)
    } else {
        Some(Error::Sheet(text))
    }
}

pub fn read(path: &Path, size: usize, sink: &mut dyn FnMut(Batch) -> bool) -> Result<()> {
    let total = std::fs::metadata(path)?.len();
    let seen = Arc::new(AtomicU64::new(0));
    let mut file = BufReader::new(Counted {
        inner: File::open(path)?,
        seen: seen.clone(),
    });
    let first = loop {
        let buf = file.fill_buf()?;
        let Some(&b) = buf.first() else {
            return Err(Error::NoColumns);
        };
        if buf.starts_with(b"\xef\xbb\xbf") {
            file.consume(3);
        } else if b.is_ascii_whitespace() {
            file.consume(1);
        } else {
            break b;
        }
    };
    let mut out = Batcher::new(sink, size, Some(total), Unit::Bytes);
    let mut rows = Rows {
        out: &mut out,
        keys: Vec::new(),
        pending: Vec::new(),
        fixed: false,
        seen,
    };
    match first {
        b'[' => {
            let mut de = serde_json::Deserializer::from_reader(file.by_ref());
            if let Err(e) = de.deserialize_seq(Each(&mut rows)) {
                return fault(&e).map_or(Ok(()), Err);
            }
        }
        b'{' => {
            let stream =
                serde_json::Deserializer::from_reader(file.by_ref()).into_iter::<Ordered>();
            for (i, obj) in stream.enumerate() {
                let obj = match obj {
                    Ok(o) => o,
                    Err(e) => return fault(&e).map_or(Ok(()), Err),
                };
                if !rows.take(i as u64 + 1, obj) {
                    return Ok(());
                }
            }
        }
        _ => return Err(Error::JsonShape),
    }
    rows.end(total)
}
