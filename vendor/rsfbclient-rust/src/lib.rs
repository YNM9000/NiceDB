//! Firebird client implementation in pure rust
#![allow(unexpected_cfgs, dead_code)]

mod arc4;
mod blr;
mod client;
mod consts;
mod events;
mod raw;
mod srp;
mod util;
mod wire;
mod xsqlda;

pub use client::{ColumnInfo, StmtHandleData, DbHandle, RustFbClient, RustFbClientAttachmentConfig, StmtHandle, TrHandle};
pub use raw::RawValue;

#[cfg(feature = "fuzz_testing")]
pub use self::{blr::*, wire::*, xsqlda::*};
