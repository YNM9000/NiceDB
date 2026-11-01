use crate::{Check, ColumnGen, Gen, Shape, shape};

#[derive(Debug, Clone, Default, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct Facts {
    pub table: String,
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
    pub unique: bool,
    pub auto: bool,
    pub has_default: bool,
    pub check: Option<Check>,
    pub parent: Option<(u32, u32)>,
    pub next: Option<i64>,
}

fn words(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for c in name.chars() {
        if c == '_' || c == ' ' || c == '-' {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
            continue;
        }
        if c.is_uppercase() && prev_lower && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        prev_lower = c.is_lowercase();
        cur.extend(c.to_lowercase());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn has(w: &[String], keys: &[&str]) -> bool {
    w.iter().any(|x| keys.iter().any(|k| x == k))
}

fn starts(w: &[String], keys: &[&str]) -> bool {
    w.iter().any(|x| keys.iter().any(|k| x.starts_with(k)))
}

const PEOPLE: &[&str] = &[
    "client",
    "utent",
    "person",
    "dipendent",
    "contatt",
    "anagraf",
    "user",
    "customer",
    "employee",
    "autor",
    "author",
    "student",
    "soci",
    "pazient",
    "patient",
    "member",
    "iscritt",
    "agent",
    "tecnic",
    "medic",
    "docent",
    "people",
    "account",
];
const FIRMS: &[&str] = &[
    "fornitor", "aziend", "societ", "ditt", "compan", "supplier", "vendor", "impres", "partner",
];

#[allow(clippy::too_many_lines)]
fn text(w: &[String], table: &str, max: Option<u32>) -> Gen {
    let t = table.to_lowercase();
    let people = PEOPLE.iter().any(|k| t.contains(k));
    let firms = FIRMS.iter().any(|k| t.contains(k));
    let joined = w.join("_");
    if has(w, &["email", "mail", "pec"]) || joined.contains("e_mail") {
        return Gen::Email;
    }
    if joined.contains("codice_fiscale")
        || joined.contains("cod_fisc")
        || has(w, &["cf", "codfisc", "codicefiscale", "fiscal"])
    {
        return Gen::CodiceFiscale;
    }
    if joined.contains("partita_iva")
        || joined.contains("p_iva")
        || has(w, &["piva", "vat", "partitaiva"])
    {
        return Gen::PartitaIva;
    }
    if has(w, &["nominativo", "fullname"])
        || joined.contains("full_name")
        || joined.contains("nome_completo")
    {
        return Gen::FullName;
    }
    if has(w, &["cognome", "surname", "lastname"]) || joined.contains("last_name") {
        return Gen::LastName;
    }
    if joined.contains("first_name") || has(w, &["firstname"]) {
        return Gen::FirstName;
    }
    if joined.contains("ragione_sociale")
        || has(
            w,
            &[
                "azienda",
                "societa",
                "società",
                "company",
                "ditta",
                "fornitore",
                "denominazione",
            ],
        )
    {
        return Gen::Company;
    }
    if has(w, &["nome", "name"]) && w.len() == 1 {
        if people {
            return Gen::FirstName;
        }
        if firms {
            return Gen::Company;
        }
        return Gen::Lorem {
            min_words: 1,
            max_words: 3,
        };
    }
    if starts(
        w,
        &[
            "telefon", "tel", "phone", "cellular", "mobile", "fax", "cell",
        ],
    ) {
        return Gen::Phone;
    }
    if starts(w, &["indirizz", "address", "street"]) || has(w, &["via"]) {
        return Gen::Street;
    }
    if has(
        w,
        &[
            "citta",
            "città",
            "comune",
            "city",
            "localita",
            "località",
            "town",
        ],
    ) {
        return Gen::City;
    }
    if has(
        w,
        &[
            "provincia",
            "prov",
            "province",
            "pr",
            "county",
            "region",
            "regione",
        ],
    ) {
        return Gen::Province;
    }
    if has(w, &["cap", "zip", "postal", "postcode"]) {
        return Gen::PostalCode;
    }
    if has(w, &["nazione", "paese", "country", "stato_estero"]) {
        return Gen::Pick {
            values: [
                "Italia", "Italia", "Italia", "Svizzera", "Francia", "Austria",
            ]
            .map(String::from)
            .to_vec(),
            weights: Vec::new(),
        };
    }
    if has(w, &["sesso", "sex", "genere", "gender"]) {
        return Gen::Pick {
            values: vec!["M".into(), "F".into()],
            weights: Vec::new(),
        };
    }
    if has(w, &["uuid", "guid"]) {
        return Gen::Uuid;
    }
    if max.is_some_and(|m| m <= 12)
        || starts(
            w,
            &[
                "codic", "code", "sku", "barcode", "ean", "targa", "matricol", "serial",
            ],
        )
    {
        return Gen::Code {
            len: max.unwrap_or(8).min(10),
        };
    }
    if starts(
        w,
        &[
            "note", "descriz", "descript", "comment", "testo", "text", "messagg", "message",
            "body", "oggetto", "subject", "titolo", "title",
        ],
    ) {
        let long = !starts(w, &["oggetto", "subject", "titolo", "title"]);
        return Gen::Lorem {
            min_words: if long { 4 } else { 2 },
            max_words: if long { 14 } else { 6 },
        };
    }
    if people && max.is_none_or(|m| m >= 20) {
        return Gen::Lorem {
            min_words: 1,
            max_words: 4,
        };
    }
    Gen::Lorem {
        min_words: 1,
        max_words: if max.is_some_and(|m| m < 40) { 3 } else { 6 },
    }
}

fn integer(w: &[String], min: i64, max: i64) -> Gen {
    let (lo, hi) = if has(w, &["anno", "year"]) {
        (1990, 2026)
    } else if has(w, &["eta", "età", "age"]) {
        (18, 90)
    } else if starts(w, &["qta", "quant", "pezzi", "qty"]) {
        (1, 100)
    } else if starts(w, &["giorn", "days"]) {
        (1, 60)
    } else if starts(w, &["mese", "month"]) {
        (1, 12)
    } else if starts(
        w,
        &[
            "ordin", "posiz", "riga", "sort", "rank", "priorit", "livell", "level",
        ],
    ) {
        (1, 10)
    } else {
        (1, 1000)
    };
    Gen::Integer {
        min: lo.clamp(min, max),
        max: hi.clamp(min, max),
    }
}

fn amount(w: &[String]) -> (f64, f64) {
    if starts(
        w,
        &[
            "prezz",
            "importo",
            "totale",
            "price",
            "amount",
            "cost",
            "imponibil",
            "saldo",
            "total",
            "valore",
            "stipend",
            "salary",
        ],
    ) {
        (1.0, 2000.0)
    } else if starts(
        w,
        &[
            "scont", "perc", "aliquot", "iva", "discount", "rate", "tasso",
        ],
    ) {
        (0.0, 50.0)
    } else if starts(w, &["peso", "weight", "kg"]) {
        (0.1, 100.0)
    } else if starts(w, &["lat"]) {
        (36.6, 47.1)
    } else if starts(w, &["lon", "lng"]) {
        (6.6, 18.5)
    } else {
        (0.0, 1000.0)
    }
}

fn dates(w: &[String]) -> (&'static str, &'static str) {
    if starts(w, &["nascit", "birth", "nato", "dob"]) {
        ("1945-01-01", "2005-12-31")
    } else if starts(
        w,
        &[
            "scadenz", "due", "expir", "fine", "end", "consegn", "deliver",
        ],
    ) {
        ("2025-01-01", "2027-12-31")
    } else {
        ("2022-01-01", "2026-06-30")
    }
}

fn bounded(g: Gen, check: Option<&Check>, scale: u32) -> Gen {
    let Some(Check::Range {
        min,
        max,
        min_open,
        max_open,
    }) = check
    else {
        return g;
    };
    let step = 10f64.powi(-i32::try_from(scale).unwrap_or(0));
    let lo = min.map(|v| if *min_open { v + step } else { v });
    let hi = max.map(|v| if *max_open { v - step } else { v });
    match g {
        #[allow(clippy::cast_possible_truncation)]
        Gen::Integer { min: a, max: b } => {
            let a2 = lo.map_or(a, |v| a.max(v.ceil() as i64));
            let b2 = hi.map_or(b, |v| b.min(v.floor() as i64));
            let a2 = if a2 > b2 {
                lo.map_or(b2, |v| v.ceil() as i64)
            } else {
                a2
            };
            let b2 = if a2 > b2 {
                hi.map_or(a2, |v| v.floor() as i64)
            } else {
                b2
            };
            Gen::Integer {
                min: a2,
                max: b2.max(a2),
            }
        }
        Gen::Decimal {
            min: a,
            max: b,
            scale,
        } => {
            let a2 = lo.map_or(a, |v| a.max(v));
            let b2 = hi.map_or(b, |v| b.min(v));
            let (a2, b2) = if a2 > b2 {
                (lo.unwrap_or(b2), hi.unwrap_or(a2))
            } else {
                (a2, b2)
            };
            Gen::Decimal {
                min: a2,
                max: b2.max(a2),
                scale,
            }
        }
        g => g,
    }
}

pub fn guess(f: &Facts) -> ColumnGen {
    let w = words(&f.name);
    let s = shape(&f.type_name);
    let max_len = match s {
        Shape::Text { max } => max,
        _ => None,
    };
    let mut null_ratio = if f.nullable && !f.unique { 0.1 } else { 0.0 };
    let generator = if f.auto {
        Gen::Auto
    } else if let Some((fk, column)) = f.parent {
        null_ratio = 0.0;
        Gen::Parent { fk, column }
    } else if let Some(Check::Values { values }) = &f.check {
        Gen::Pick {
            values: values.clone(),
            weights: Vec::new(),
        }
    } else if let Shape::Enum { values } = &s {
        Gen::Pick {
            values: values.clone(),
            weights: Vec::new(),
        }
    } else if f.has_default {
        Gen::Auto
    } else {
        by_shape(f, &w, &s)
    };
    if matches!(generator, Gen::Auto | Gen::Null) {
        null_ratio = 0.0;
    }
    ColumnGen {
        name: f.name.clone(),
        generator,
        null_ratio,
        max_len,
        locale: None,
        dist: crate::Dist::Uniform,
    }
}

fn by_shape(f: &Facts, w: &[String], s: &Shape) -> Gen {
    match s {
        Shape::Int { min, max } => {
            if f.unique {
                return Gen::Sequence {
                    start: f.next.unwrap_or(1).max(1),
                };
            }
            if has(w, &["attivo", "active", "abilitato", "enabled", "flag"]) && *max >= 1 {
                return Gen::Integer { min: 0, max: 1 };
            }
            bounded(integer(w, *min, *max), f.check.as_ref(), 0)
        }
        Shape::Decimal { precision, scale } => {
            let (lo, hi) = amount(w);
            let top =
                10f64.powi(i32::try_from(precision.saturating_sub(*scale)).unwrap_or(9)) - 1.0;
            let g = Gen::Decimal {
                min: lo.min(top),
                max: hi.min(top),
                scale: *scale,
            };
            bounded(g, f.check.as_ref(), *scale)
        }
        Shape::Float => {
            let (lo, hi) = amount(w);
            bounded(
                Gen::Decimal {
                    min: lo,
                    max: hi,
                    scale: 2,
                },
                f.check.as_ref(),
                2,
            )
        }
        Shape::Bool => Gen::Boolean,
        Shape::Date if starts(w, &["nascit", "birth", "nato", "dob"]) => Gen::BirthDate,
        Shape::Date => {
            let (from, to) = dates(w);
            Gen::Date {
                from: from.into(),
                to: to.into(),
            }
        }
        Shape::DateTime => {
            let (from, to) = dates(w);
            Gen::DateTime {
                from: from.into(),
                to: to.into(),
            }
        }
        Shape::Time => Gen::Time,
        Shape::Uuid => Gen::Uuid,
        Shape::Json => Gen::Json,
        Shape::Text { max } => {
            if f.unique {
                return match text(w, &f.table, *max) {
                    g @ (Gen::Email
                    | Gen::CodiceFiscale
                    | Gen::PartitaIva
                    | Gen::Uuid
                    | Gen::Code { .. }) => g,
                    _ => Gen::Code {
                        len: max.unwrap_or(10).clamp(1, 10),
                    },
                };
            }
            text(w, &f.table, *max)
        }
        Shape::Binary | Shape::Other | Shape::Enum { .. } => {
            if f.nullable {
                Gen::Null
            } else {
                text(w, &f.table, None)
            }
        }
    }
}
