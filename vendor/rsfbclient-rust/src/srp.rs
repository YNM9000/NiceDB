//! SRP client implementation for the firebird authentication (Wire Protocol 13)

#![allow(clippy::many_single_char_names)]

use std::{error, fmt, marker::PhantomData};

use digest::{Digest, OutputSizeUser};
use generic_array::GenericArray;
use lazy_static::lazy_static;
use num_bigint::BigUint;
use sha1::Sha1;

lazy_static! {
    /// Srp Group used by the firebird server
    pub static ref SRP_GROUP: SrpGroup = SrpGroup {
        n: BigUint::from_bytes_be(&[
            230, 125, 46, 153, 75, 47, 144, 12, 63, 65, 240, 143, 91, 178, 98, 126, 208, 212, 158,
            225, 254, 118, 122, 82, 239, 205, 86, 92, 214, 231, 104, 129, 44, 62, 30, 156, 232,
            240, 168, 190, 166, 203, 19, 205, 41, 221, 235, 247, 169, 109, 74, 147, 181, 93, 72,
            141, 240, 153, 161, 92, 137, 220, 176, 100, 7, 56, 235, 44, 189, 217, 168, 247, 186,
            181, 97, 171, 27, 13, 193, 198, 205, 171, 243, 3, 38, 74, 8, 209, 188, 169, 50, 209,
            241, 238, 66, 139, 97, 157, 151, 15, 52, 42, 186, 154, 101, 121, 59, 139, 47, 4, 26,
            229, 54, 67, 80, 193, 111, 115, 95, 86, 236, 188, 168, 123, 213, 123, 41, 231,
        ]),
        g: BigUint::from_bytes_be(&[2]),
    };
}

/// SRP client state before handshake with the server.
pub struct SrpClient<'a, D: Digest> {
    params: &'a SrpGroup,

    a: BigUint,
    a_pub: BigUint,

    d: PhantomData<D>,
}

/// SRP client state after handshake with the server.
pub struct SrpClientVerifier<D: Digest> {
    proof: GenericArray<u8, D::OutputSize>,
    // Firebird hashes this with SHA1 for some reason
    key: GenericArray<u8, <Sha1 as OutputSizeUser>::OutputSize>,
}

/// Compute user private key as described in the RFC 5054. Consider using proper
/// password hashing algorithm instead.
pub fn srp_private_key<D: Digest>(
    username: &[u8],
    password: &[u8],
    salt: &[u8],
) -> GenericArray<u8, D::OutputSize> {
    let p = D::new()
        .chain_update(username)
        .chain_update(b":")
        .chain_update(password)
        .finalize();

    D::new().chain_update(salt).chain_update(&p).finalize()
}

impl<'a, D: Digest> SrpClient<'a, D> {
    /// Create new SRP client instance.
    pub fn new(a: &[u8], params: &'a SrpGroup) -> Self {
        let a = BigUint::from_bytes_be(a);
        let a_pub = params.powm(&a);

        Self {
            params,
            a,
            a_pub,
            d: Default::default(),
        }
    }

    // Firebird hashes this with SHA1 for some reason
    fn calc_key(
        &self,
        b_pub: &BigUint,
        x: &BigUint,
        u: &BigUint,
    ) -> GenericArray<u8, <Sha1 as OutputSizeUser>::OutputSize> {
        let n = &self.params.n;
        let k = self.params.compute_k::<Sha1>();
        let interm = (k * self.params.powm(x)) % n;
        // Because we do operation in modulo N we can get: (kv + g^b) < kv
        let v = if *b_pub > interm {
            (b_pub - &interm) % n
        } else {
            (n + b_pub - &interm) % n
        };
        // S = |B - kg^x| ^ (a + ux)
        let s = powm(&v, &(&self.a + (u * x) % n), n);
        Sha1::digest(&s.to_bytes_be())
    }

    /// Process server reply to the handshake.
    pub fn process_reply(
        self,
        user: &[u8],
        salt: &[u8],
        private_key: &[u8],
        b_pub: &[u8],
    ) -> Result<SrpClientVerifier<D>, SrpAuthError> {
        let u = {
            BigUint::from_bytes_be(
                // Firebird hashes this with SHA1 for some reason
                &Sha1::new()
                    .chain_update(&self.a_pub.to_bytes_be())
                    .chain_update(b_pub)
                    .finalize(),
            )
        };

        let b_pub = BigUint::from_bytes_be(b_pub);

        // Safeguard against malicious B
        if &b_pub % &self.params.n == BigUint::default() {
            return Err(SrpAuthError {
                description: "Malicious b_pub value",
            });
        }

        let x = BigUint::from_bytes_be(private_key);
        let key = self.calc_key(&b_pub, &x, &u);
        // M = H(pow(H(N), H(g)) % N | H(U) | s | A | B | K)
        let proof = {
            let hn = {
                let n = &self.params.n;

                // Firebird hashes this with SHA1 for some reason
                BigUint::from_bytes_be(&Sha1::new().chain_update(n.to_bytes_be()).finalize())
            };
            let hg = {
                let g = &self.params.g;

                // Firebird hashes this with SHA1 for some reason
                BigUint::from_bytes_be(&Sha1::new().chain_update(g.to_bytes_be()).finalize())
            };
            // Firebird hashes this with SHA1 for some reason
            let hu = Sha1::new().chain_update(user).finalize();

            D::new()
                .chain_update((hn.modpow(&hg, &self.params.n)).to_bytes_be())
                .chain_update(hu)
                .chain_update(salt)
                .chain_update(&self.a_pub.to_bytes_be())
                .chain_update(&b_pub.to_bytes_be())
                .chain_update(&key)
                .finalize()
        };

        Ok(SrpClientVerifier { proof, key })
    }

    /// Get public ephemeral value for handshaking with the server.
    pub fn get_a_pub(&self) -> Vec<u8> {
        self.a_pub.to_bytes_be()
    }
}

impl<D: Digest> SrpClientVerifier<D> {
    /// Get shared secret key without authenticating server, e.g. for using with
    /// authenticated encryption modes. DO NOT USE this method without
    /// some kind of secure authentication
    pub fn get_key(self) -> GenericArray<u8, <Sha1 as OutputSizeUser>::OutputSize> {
        self.key
    }

    /// Verification data for sending to the server.
    pub fn get_proof(&self) -> GenericArray<u8, D::OutputSize> {
        self.proof.clone()
    }
}

pub fn powm(base: &BigUint, exp: &BigUint, modulus: &BigUint) -> BigUint {
    let zero = BigUint::from(0u32);
    let one = BigUint::from(1u32);
    let two = BigUint::from(2u32);
    let mut exp = exp.clone();
    let mut result = one.clone();
    let mut base = base % modulus;

    while exp > zero {
        if &exp % &two == one {
            result = (result * &base) % modulus;
        }
        exp >>= 1;
        base = (&base * &base) % modulus;
    }
    result
}

/// SRP authentication error.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct SrpAuthError {
    pub(crate) description: &'static str,
}

impl fmt::Display for SrpAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SRP authentication error")
    }
}

impl error::Error for SrpAuthError {
    fn description(&self) -> &str {
        self.description
    }
}

/// Group used for SRP computations
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SrpGroup {
    /// A large safe prime (N = 2q+1, where q is prime)
    pub n: BigUint,
    /// A generator modulo N
    pub g: BigUint,
}

impl SrpGroup {
    pub(crate) fn powm(&self, v: &BigUint) -> BigUint {
        powm(&self.g, v, &self.n)
    }

    /// Compute `k` with given hash function and return SRP parameters
    pub(crate) fn compute_k<D: Digest>(&self) -> BigUint {
        let n = self.n.to_bytes_be();
        let g_bytes = self.g.to_bytes_be();
        let mut buf = vec![0u8; n.len()];
        let l = n.len() - g_bytes.len();
        buf[l..].copy_from_slice(&g_bytes);

        BigUint::from_bytes_be(&D::new().chain_update(&n).chain_update(&buf).finalize())
    }
}
