use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use nd_driver::Target;
use serde::Serialize;

use crate::session::{Session, dropped};
use crate::{Core, CoreError};

const RETRY_MAX: Duration = Duration::from_mins(1);

#[derive(Debug, Clone)]
pub(crate) struct Dormant {
    pub connection_id: String,
    pub target: Target,
    pub mode: crate::tx::CommitMode,
    pub transaction_lost: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    Lost {
        session_id: String,
        error: CoreError,
    },
    Reconnected {
        session_id: String,
        transaction_lost: bool,
        server: nd_driver::ServerInfo,
    },
    IdleClosed {
        session_id: String,
        transaction_lost: bool,
    },
}

impl Session {
    fn retry_due(&self) -> bool {
        self.retry.lock().is_ok_and(|r| r.0 <= Instant::now())
    }

    fn back_off(&self) {
        if let Ok(mut r) = self.retry.lock() {
            *r = (Instant::now() + r.1, (r.1 * 2).min(RETRY_MAX));
        }
    }
}

impl Core {
    pub async fn upkeep(&self) -> Vec<SessionEvent> {
        let sessions: Vec<(String, Arc<Session>)> = match Self::lock(&self.sessions) {
            Ok(map) => map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            Err(_) => return Vec::new(),
        };
        let mut events = Vec::new();
        for (id, s) in sessions {
            if s.busy.load(Ordering::Acquire) {
                continue;
            }
            let options = s.options();
            if s.lost.load(Ordering::Acquire) {
                if options.reconnect && s.retry_due() {
                    events.extend(self.revive(&id, &s, false).await);
                }
                continue;
            }
            if options.idle_minutes > 0
                && s.idle_for() >= Duration::from_mins(u64::from(options.idle_minutes))
            {
                events.extend(self.doze(&id, &s));
                continue;
            }
            if options.keepalive_secs == 0
                || !s.networked()
                || s.quiet_for() < Duration::from_secs(u64::from(options.keepalive_secs))
            {
                continue;
            }
            match s.ping().await {
                Err(e) if dropped(&e) && !s.busy.load(Ordering::Acquire) => {
                    tracing::info!(error = %e, "session dropped, dialing again");
                    s.mark_lost();
                    if options.reconnect {
                        events.extend(self.revive(&id, &s, true).await);
                    } else {
                        events.push(SessionEvent::Lost {
                            session_id: id,
                            error: e.into(),
                        });
                    }
                }
                _ => s.pinged(),
            }
        }
        events
    }

    async fn revive(&self, id: &str, s: &Session, first: bool) -> Option<SessionEvent> {
        let transaction_lost = s.tx_lost.load(Ordering::Acquire);
        match self.reconnect(id).await {
            Ok(info) => Some(SessionEvent::Reconnected {
                session_id: id.to_owned(),
                transaction_lost,
                server: info.server,
            }),
            Err(error) => {
                s.back_off();
                first.then(|| SessionEvent::Lost {
                    session_id: id.to_owned(),
                    error,
                })
            }
        }
    }

    fn doze(&self, id: &str, s: &Session) -> Option<SessionEvent> {
        let removed = Self::lock(&self.sessions).ok()?.remove(id)?;
        let transaction_lost = s.tx_open();
        Self::lock(&self.dormant).ok()?.insert(
            id.to_owned(),
            Dormant {
                connection_id: removed.connection_id.clone(),
                target: removed.target.clone(),
                mode: s.commit_mode(),
                transaction_lost: false,
            },
        );
        Some(SessionEvent::IdleClosed {
            session_id: id.to_owned(),
            transaction_lost,
        })
    }

    pub fn is_dormant(&self, session_id: &str) -> bool {
        Self::lock(&self.dormant).is_ok_and(|d| d.contains_key(session_id))
    }
}

impl Core {
    #[doc(hidden)]
    pub fn age_for_test(&self, session_id: &str, by: Duration) {
        if let Ok(s) = self.session(session_id)
            && let Ok(mut t) = s.last_used.lock()
            && let Some(back) = t.checked_sub(by)
        {
            *t = back;
        }
    }
}
