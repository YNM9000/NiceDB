pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

pub fn parse_date(s: &str) -> Option<i64> {
    let s = s.trim();
    let mut parts = s.get(..10)?.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then(|| days_from_civil(y, m, d))
}

pub fn format_date(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn letters(s: &str) -> (Vec<char>, Vec<char>) {
    let plain: String = s
        .chars()
        .map(|c| match c {
            'à' | 'á' => 'A',
            'è' | 'é' => 'E',
            'ì' | 'í' => 'I',
            'ò' | 'ó' => 'O',
            'ù' | 'ú' => 'U',
            c => c.to_ascii_uppercase(),
        })
        .filter(char::is_ascii_uppercase)
        .collect();
    plain.chars().partition(|c| !"AEIOU".contains(*c))
}

fn three(mut out: Vec<char>, vowels: &[char]) -> String {
    out.extend_from_slice(vowels);
    out.resize(out.len().max(3), 'X');
    out[..3].iter().collect()
}

fn surname_code(s: &str) -> String {
    let (cons, vow) = letters(s);
    three(cons, &vow)
}

fn name_code(s: &str) -> String {
    let (cons, vow) = letters(s);
    if cons.len() >= 4 {
        return [cons[0], cons[2], cons[3]].iter().collect();
    }
    three(cons, &vow)
}

const MONTHS: &[u8; 12] = b"ABCDEHLMPRST";
const ODD: [u32; 36] = [
    1, 0, 5, 7, 9, 13, 15, 17, 19, 21, 1, 0, 5, 7, 9, 13, 15, 17, 19, 21, 2, 4, 18, 20, 11, 3, 6,
    8, 12, 14, 16, 10, 22, 25, 24, 23,
];

fn value(c: char) -> usize {
    match c {
        '0'..='9' => c as usize - '0' as usize,
        'A'..='Z' => c as usize - 'A' as usize + 10,
        _ => 0,
    }
}

pub fn cf_check(first15: &str) -> char {
    let sum: u32 = first15
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let v = value(c);
            if i % 2 == 0 {
                ODD[v]
            } else {
                u32::try_from(if v >= 10 { v - 10 } else { v }).unwrap_or(0)
            }
        })
        .sum();
    char::from(b'A' + u8::try_from(sum % 26).unwrap_or(0))
}

pub struct Person<'a> {
    pub first: &'a str,
    pub last: &'a str,
    pub female: bool,
    pub born: i64,
    pub place: &'a str,
}

pub fn codice_fiscale(p: &Person<'_>) -> String {
    let (y, m, d) = civil_from_days(p.born);
    let day = if p.female { d + 40 } else { d };
    let month = char::from(MONTHS[(m as usize).clamp(1, 12) - 1]);
    let head = format!(
        "{}{}{:02}{month}{day:02}{}",
        surname_code(p.last),
        name_code(p.first),
        y.rem_euclid(100),
        p.place
    );
    let check = cf_check(&head);
    format!("{head}{check}")
}

pub fn piva_check(first10: &[u8]) -> u8 {
    let sum: u32 = first10
        .iter()
        .enumerate()
        .map(|(i, &d)| {
            let d = u32::from(d);
            if i % 2 == 0 {
                d
            } else {
                let x = d * 2;
                if x > 9 { x - 9 } else { x }
            }
        })
        .sum();
    u8::try_from((10 - sum % 10) % 10).unwrap_or(0)
}

pub fn partita_iva(body: u32, office: u32) -> String {
    let head = format!("{:07}{:03}", body % 10_000_000, office.clamp(1, 100));
    let digits: Vec<u8> = head.bytes().map(|b| b - b'0').collect();
    format!("{head}{}", piva_check(&digits))
}
