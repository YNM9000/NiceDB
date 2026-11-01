use std::fmt::Write as _;

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub(crate) struct Credentials {
    pub key_id: String,
    pub secret: zeroize::Zeroizing<String>,
    pub token: Option<String>,
}

fn hmac(key: &[u8], data: &str) -> Vec<u8> {
    let Ok(mut mac) = HmacSha256::new_from_slice(key) else {
        return Vec::new();
    };
    mac.update(data.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

pub(crate) fn signing_key(secret: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k = hmac(format!("AWS4{secret}").as_bytes(), date);
    let k = hmac(&k, region);
    let k = hmac(&k, service);
    hmac(&k, "aws4_request")
}

pub(crate) fn amz_date(secs: u64) -> String {
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}{m:02}{d:02}T{:02}{:02}{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

pub(crate) fn headers(
    creds: &Credentials,
    region: &str,
    host: &str,
    target: &str,
    body: &[u8],
    now_secs: u64,
) -> Vec<(&'static str, String)> {
    let stamp = amz_date(now_secs);
    let date = &stamp[..8];
    let mut signed: Vec<(&'static str, String)> = vec![
        ("host", host.to_owned()),
        ("x-amz-date", stamp.clone()),
        ("x-amz-target", target.to_owned()),
    ];
    if let Some(t) = &creds.token {
        signed.push(("x-amz-security-token", t.clone()));
    }
    signed.sort_by(|a, b| a.0.cmp(b.0));
    let names = signed.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(";");
    let canonical_headers = signed.iter().fold(String::new(), |mut out, (k, v)| {
        let _ = writeln!(out, "{k}:{}", v.trim());
        out
    });
    let canonical = format!(
        "POST\n/\n\n{canonical_headers}\n{names}\n{}",
        hex::encode(Sha256::digest(body))
    );
    let scope = format!("{date}/{region}/dynamodb/aws4_request");
    let to_sign = format!(
        "AWS4-HMAC-SHA256\n{stamp}\n{scope}\n{}",
        hex::encode(Sha256::digest(canonical.as_bytes()))
    );
    let key = signing_key(&creds.secret, date, region, "dynamodb");
    let signature = hex::encode(hmac(&key, &to_sign));
    let mut out: Vec<(&'static str, String)> =
        signed.into_iter().filter(|(k, _)| *k != "host").collect();
    out.push((
        "authorization",
        format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={names}, Signature={signature}",
            creds.key_id
        ),
    ));
    out
}
