use nd_driver::{DriverError, KeyPage, KeyRead, KeyScan, KeyValue, KeyWrite, Outcome, Report};

use crate::guard::{Access, claim};
use crate::{Core, CoreError, Result};

impl Core {
    pub async fn scan_keys(&self, session_id: &str, scan: &KeyScan) -> Result<KeyPage> {
        let session = self.live(session_id).await?;
        Ok(session.conn.scan_keys(scan).await?)
    }

    pub async fn read_key(&self, session_id: &str, read: &KeyRead) -> Result<KeyValue> {
        let session = self.live(session_id).await?;
        Ok(session.conn.read_key(read).await?)
    }

    pub async fn write_key(
        &self,
        session_id: &str,
        write: &KeyWrite,
        confirm: Option<&str>,
    ) -> Result<Outcome> {
        let session = self.live(session_id).await?;
        if !session.caps.editable {
            return Err(CoreError::Driver(DriverError::query(
                "keys of this engine cannot be edited",
            )));
        }
        let _busy = claim(&session)?;
        self.admit(&session, Access::Write { confirm }).await?;
        let shown = session.conn.describe_key_write(write);
        let out = session.conn.write_key(write).await;
        match &out {
            Ok(o) => self.audit(&session, &shown, o.rows_affected, None),
            Err(e) => self.audit(&session, &shown, None, Some(e.to_string())),
        }
        Ok(out?)
    }

    pub async fn server_report(&self, session_id: &str) -> Result<Report> {
        let session = self.live(session_id).await?;
        Ok(session.conn.report().await?)
    }
}
