use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

/// A flat semaphore on job *count* is insufficient: submissions request
/// different CPU (`cpu_limit_cores`) and memory (`memory_limit_kb`), so N
/// concurrent jobs can oversubscribe a host at low N. This tracks a
/// weighted budget instead, acquired before any containerd work.
#[derive(Clone)]
pub struct AdmissionControl {
    inner: Arc<Mutex<Budget>>,
    notify: Arc<Notify>,
}

struct Budget {
    cpu_cores_available: f64,
    memory_bytes_available: u64,
}

impl AdmissionControl {
    pub fn new(total_cpu_cores: f64, total_memory_bytes: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Budget {
                cpu_cores_available: total_cpu_cores,
                memory_bytes_available: total_memory_bytes,
            })),
            notify: Arc::new(Notify::new()),
        }
    }

    pub async fn acquire(&self, cpu_cores: f64, memory_bytes: u64) -> AdmissionGuard {
        loop {
            {
                let mut b = self.inner.lock().await;
                if b.cpu_cores_available >= cpu_cores && b.memory_bytes_available >= memory_bytes {
                    b.cpu_cores_available -= cpu_cores;
                    b.memory_bytes_available -= memory_bytes;
                    return AdmissionGuard { control: self.clone(), cpu_cores, memory_bytes };
                }
            }
            self.notify.notified().await;
        }
    }
}

pub struct AdmissionGuard {
    control: AdmissionControl,
    cpu_cores: f64,
    memory_bytes: u64,
}

impl Drop for AdmissionGuard {
    fn drop(&mut self) {
        // Pure in-memory arithmetic, no I/O — safe to release from a
        // spawned task inside Drop, unlike the containerd gRPC cleanup in
        // lifecycle.rs, which must be an explicit `.await`.
        let control = self.control.clone();
        let (cpu, mem) = (self.cpu_cores, self.memory_bytes);
        tokio::spawn(async move {
            let mut b = control.inner.lock().await;
            b.cpu_cores_available += cpu;
            b.memory_bytes_available += mem;
            control.notify.notify_waiters();
        });
    }
}
