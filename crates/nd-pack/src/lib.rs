use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub const FORMAT: &str = "nicedb-package";
pub const VERSION: u32 = 1;

const KDF: &str = "argon2id";
const CIPHER: &str = "xchacha20poly1305";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("not a NiceDB package")]
    Malformed,
    #[error("package format not supported by this build")]
    Unsupported,
    #[error("key derivation cost out of bounds")]
    Cost,
    #[error("wrong passphrase or altered package")]
    Decrypt,
    #[error("the passphrase is empty")]
    EmptyPassphrase,
    #[error("system randomness unavailable")]
    Random,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cost {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Cost {
    pub const DEFAULT: Self = Self {
        memory_kib: 64 * 1024,
        iterations: 3,
        parallelism: 1,
    };

    fn bounded(self) -> bool {
        (8 * self.parallelism..=1024 * 1024).contains(&self.memory_kib)
            && (1..=64).contains(&self.iterations)
            && (1..=16).contains(&self.parallelism)
    }
}

#[derive(Serialize, Deserialize)]
struct Kdf {
    algorithm: String,
    #[serde(flatten)]
    cost: Cost,
    salt: String,
}

#[derive(Serialize, Deserialize)]
struct Cipher {
    algorithm: String,
    nonce: String,
}

#[derive(Serialize, Deserialize)]
struct Header {
    format: String,
    version: u32,
    kdf: Kdf,
    cipher: Cipher,
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    #[serde(flatten)]
    header: Header,
    ciphertext: String,
}

#[derive(Deserialize)]
struct Probe {
    format: Option<String>,
}

fn random<const N: usize>() -> Result<[u8; N]> {
    let mut out = [0u8; N];
    getrandom::fill(&mut out).map_err(|_| Error::Random)?;
    Ok(out)
}

fn derive(passphrase: &str, salt: &[u8], cost: Cost) -> Result<Zeroizing<[u8; 32]>> {
    if passphrase.is_empty() {
        return Err(Error::EmptyPassphrase);
    }
    if !cost.bounded() {
        return Err(Error::Cost);
    }
    let params = Params::new(cost.memory_kib, cost.iterations, cost.parallelism, Some(32))
        .map_err(|_| Error::Cost)?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), salt, key.as_mut())
        .map_err(|_| Error::Cost)?;
    Ok(key)
}

fn aad(header: &Header) -> Result<Vec<u8>> {
    serde_json::to_vec(header).map_err(|_| Error::Malformed)
}

pub fn is_sealed(file: &[u8]) -> bool {
    serde_json::from_slice::<Probe>(file).is_ok_and(|p| p.format.as_deref() == Some(FORMAT))
}

pub fn seal(plaintext: &[u8], passphrase: &str) -> Result<Vec<u8>> {
    seal_with(plaintext, passphrase, Cost::DEFAULT)
}

pub fn seal_with(plaintext: &[u8], passphrase: &str, cost: Cost) -> Result<Vec<u8>> {
    let salt = random::<SALT_LEN>()?;
    let nonce = random::<NONCE_LEN>()?;
    let key = derive(passphrase, &salt, cost)?;
    let header = Header {
        format: FORMAT.into(),
        version: VERSION,
        kdf: Kdf {
            algorithm: KDF.into(),
            cost,
            salt: STANDARD.encode(salt),
        },
        cipher: Cipher {
            algorithm: CIPHER.into(),
            nonce: STANDARD.encode(nonce),
        },
    };
    let aad = aad(&header)?;
    let sealed = XChaCha20Poly1305::new(key.as_ref().into())
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| Error::Decrypt)?;
    serde_json::to_vec_pretty(&Envelope {
        header,
        ciphertext: STANDARD.encode(sealed),
    })
    .map_err(|_| Error::Malformed)
}

pub fn open(file: &[u8], passphrase: &str) -> Result<Zeroizing<Vec<u8>>> {
    let env: Envelope = serde_json::from_slice(file).map_err(|_| Error::Malformed)?;
    let h = &env.header;
    if h.format != FORMAT {
        return Err(Error::Malformed);
    }
    if h.version != VERSION || h.kdf.algorithm != KDF || h.cipher.algorithm != CIPHER {
        return Err(Error::Unsupported);
    }
    let decode = |s: &str| STANDARD.decode(s).map_err(|_| Error::Malformed);
    let salt = decode(&h.kdf.salt)?;
    let nonce = decode(&h.cipher.nonce)?;
    let sealed = decode(&env.ciphertext)?;
    if salt.len() < 8 || nonce.len() != NONCE_LEN {
        return Err(Error::Malformed);
    }
    let key = derive(passphrase, &salt, h.kdf.cost)?;
    let aad = aad(h)?;
    XChaCha20Poly1305::new(key.as_ref().into())
        .decrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &sealed,
                aad: &aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| Error::Decrypt)
}
