use serde_json::{Map, Value, json};

fn number(n: &str) -> Value {
    let n = n.trim();
    if let Ok(i) = n.parse::<i64>()
        && i.to_string() == n
        && i.unsigned_abs() < (1 << 53)
    {
        return Value::from(i);
    }
    if let Ok(f) = n.parse::<f64>()
        && f.is_finite()
        && let Some(num) = serde_json::Number::from_f64(f)
        && num.to_string() == n
    {
        return Value::Number(num);
    }
    json!({ "$numberDecimal": n })
}

fn binary(b64: &Value) -> Value {
    json!({ "$binary": { "base64": b64, "subType": "00" } })
}

pub(crate) fn to_json(av: &Value) -> Value {
    let Some((kind, v)) = av.as_object().and_then(|o| o.iter().next()) else {
        return Value::Null;
    };
    match kind.as_str() {
        "S" | "BOOL" => v.clone(),
        "N" => v.as_str().map_or(Value::Null, number),
        "B" => binary(v),
        "M" => Value::Object(
            v.as_object()
                .into_iter()
                .flatten()
                .map(|(k, x)| (k.clone(), to_json(x)))
                .collect(),
        ),
        "L" => Value::Array(v.as_array().into_iter().flatten().map(to_json).collect()),
        "SS" => json!({ "$set": v }),
        "NS" => {
            json!({ "$set": v.as_array().into_iter().flatten().filter_map(Value::as_str).map(number).collect::<Vec<_>>() })
        }
        "BS" => {
            json!({ "$set": v.as_array().into_iter().flatten().map(binary).collect::<Vec<_>>() })
        }
        _ => Value::Null,
    }
}

pub(crate) fn item_to_document(item: &Map<String, Value>, keys: &[String]) -> String {
    let mut out = String::from("{");
    let mut first = true;
    let mut push = |k: &str, v: &Value| {
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(&Value::from(k).to_string());
        out.push(':');
        out.push_str(&to_json(v).to_string());
    };
    for k in keys {
        if let Some(v) = item.get(k) {
            push(k, v);
        }
    }
    for (k, v) in item {
        if !keys.contains(k) {
            push(k, v);
        }
    }
    out.push('}');
    out
}

fn wrapped<'a>(o: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    (o.len() == 1).then(|| o.get(key)).flatten()
}

fn num_text(v: &Value) -> Option<String> {
    match v {
        Value::Number(n) => Some(n.to_string()),
        Value::Object(o) => wrapped(o, "$numberDecimal")
            .and_then(Value::as_str)
            .map(str::to_owned),
        _ => None,
    }
}

fn bin_text(v: &Value) -> Option<Value> {
    let b = wrapped(v.as_object()?, "$binary")?;
    Some(b.get("base64").cloned().unwrap_or_else(|| b.clone()))
}

pub(crate) fn from_json(v: &Value) -> Result<Value, String> {
    Ok(match v {
        Value::Null => json!({ "NULL": true }),
        Value::Bool(b) => json!({ "BOOL": b }),
        Value::Number(n) => json!({ "N": n.to_string() }),
        Value::String(s) => json!({ "S": s }),
        Value::Array(list) => {
            json!({ "L": list.iter().map(from_json).collect::<Result<Vec<_>, _>>()? })
        }
        Value::Object(o) => {
            if let Some(n) = wrapped(o, "$numberDecimal").and_then(Value::as_str) {
                return Ok(json!({ "N": n }));
            }
            if let Some(b) = bin_text(v) {
                return Ok(json!({ "B": b }));
            }
            if let Some(set) = wrapped(o, "$set").and_then(Value::as_array) {
                return set_of(set);
            }
            json!({ "M": o.iter().map(|(k, x)| Ok((k.clone(), from_json(x)?))).collect::<Result<Map<_, _>, String>>()? })
        }
    })
}

fn set_of(set: &[Value]) -> Result<Value, String> {
    if set.is_empty() {
        return Err("a set cannot be empty in DynamoDB".into());
    }
    if set.iter().all(Value::is_string) {
        return Ok(json!({ "SS": set }));
    }
    if let Some(ns) = set.iter().map(num_text).collect::<Option<Vec<_>>>() {
        return Ok(json!({ "NS": ns }));
    }
    if let Some(bs) = set.iter().map(bin_text).collect::<Option<Vec<_>>>() {
        return Ok(json!({ "BS": bs }));
    }
    Err("a set holds only strings, only numbers or only binaries".into())
}

pub(crate) fn document_to_item(text: &str) -> Result<Map<String, Value>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("not valid JSON: {e}"))?;
    let Value::Object(o) = v else {
        return Err("an item is a JSON object".into());
    };
    o.iter()
        .map(|(k, x)| Ok((k.clone(), from_json(x)?)))
        .collect()
}
