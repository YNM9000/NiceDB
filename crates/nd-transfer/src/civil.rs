pub fn date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

pub fn timestamp_micros(micros: i64) -> String {
    let secs = micros.div_euclid(1_000_000);
    let frac = micros.rem_euclid(1_000_000);
    let day = secs.div_euclid(86_400);
    let s = secs.rem_euclid(86_400);
    let clock = format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60);
    if frac == 0 {
        format!("{} {clock}", date(day))
    } else {
        format!("{} {clock}.{frac:06}", date(day))
    }
}

pub fn excel(serial: f64) -> String {
    #[allow(clippy::cast_possible_truncation)]
    let micros = ((serial - 25_569.0) * 86_400_000_000.0).round() as i64;
    if serial.fract() == 0.0 {
        date(micros.div_euclid(86_400_000_000))
    } else {
        let mut t = timestamp_micros(micros);
        if t.ends_with(":00") && micros % 1_000_000 == 0 {
            t.truncate(t.len() - 3);
        }
        t
    }
}
