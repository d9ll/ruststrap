use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
pub struct ProgressData {
    pub status: String,
    pub detail: String,
    pub downloaded: u64,
    pub total: u64,
    pub finished: bool,
    pub error: Option<String>,
}

#[derive(Clone, Default)]
pub struct Progress {
    inner: Arc<Mutex<ProgressData>>,
    cancelled: Arc<AtomicBool>,
}

impl Progress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_status(&self, status: impl Into<String>) {
        if let Ok(mut data) = self.inner.lock() {
            data.status = status.into();
        }
    }

    pub fn set_detail(&self, detail: impl Into<String>) {
        if let Ok(mut data) = self.inner.lock() {
            data.detail = detail.into();
        }
    }

    pub fn set_total(&self, total: u64) {
        if let Ok(mut data) = self.inner.lock() {
            data.total = total;
            data.downloaded = 0;
        }
    }

    pub fn add_bytes(&self, bytes: u64) {
        if let Ok(mut data) = self.inner.lock() {
            data.downloaded = data.downloaded.saturating_add(bytes);
        }
    }

    pub fn sub_bytes(&self, bytes: u64) {
        if let Ok(mut data) = self.inner.lock() {
            data.downloaded = data.downloaded.saturating_sub(bytes);
        }
    }

    pub fn finish(&self) {
        if let Ok(mut data) = self.inner.lock() {
            data.finished = true;
            data.status = "Done".into();
        }
    }

    pub fn fail(&self, error: impl Into<String>) {
        if let Ok(mut data) = self.inner.lock() {
            data.error = Some(error.into());
            data.finished = true;
        }
    }

    pub fn snapshot(&self) -> ProgressData {
        self.inner.lock().map(|d| d.clone()).unwrap_or_default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn fraction(&self) -> f32 {
        let data = self.snapshot();
        if data.total == 0 {
            0.0
        } else {
            (data.downloaded as f64 / data.total as f64).clamp(0.0, 1.0) as f32
        }
    }
}
