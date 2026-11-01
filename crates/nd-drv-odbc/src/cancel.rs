#![allow(unsafe_code)]

use std::sync::{Arc, Mutex};

use odbc_api::sys::{HStmt, SQLCancel, SqlReturn};

#[derive(Clone, Default)]
pub struct Running(Arc<Mutex<Option<usize>>>);

impl Running {
    pub fn set(&self, handle: Option<HStmt>) {
        if let Ok(mut h) = self.0.lock() {
            *h = handle.map(|p| p.0 as usize);
        }
    }

    pub fn cancel(&self) -> bool {
        let Ok(h) = self.0.lock() else {
            return false;
        };
        let Some(handle) = *h else {
            return false;
        };
        let ret = unsafe { SQLCancel(HStmt(handle as *mut std::ffi::c_void)) };
        ret == SqlReturn::SUCCESS || ret == SqlReturn::SUCCESS_WITH_INFO
    }
}
