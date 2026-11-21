use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use sysinfo::System;

#[derive(Clone, Copy, Debug, Default)]
pub struct Metrics {
    pub input_rate: f32,
    pub cpu_usage: f32,
    pub memory_fraction: f32,
}

pub struct Reactivity {
    key_times: Vec<Instant>,
    cpu_usage: Arc<AtomicU32>,
    memory_fraction: Arc<AtomicU32>,
    stop_metrics: Arc<AtomicBool>,
    metrics_worker: Option<JoinHandle<()>>,
}

impl Reactivity {
    pub fn start() -> Self {
        let cpu_usage = Arc::new(AtomicU32::new(0.0_f32.to_bits()));
        let memory_fraction = Arc::new(AtomicU32::new(0.0_f32.to_bits()));
        let cpu_metrics = Arc::clone(&cpu_usage);
        let memory_metrics = Arc::clone(&memory_fraction);
        let stop_metrics = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop_metrics);
        let metrics_worker = thread::spawn(move || {
            let mut system = System::new();
            while !worker_stop.load(Ordering::Acquire) {
                system.refresh_cpu_usage();
                system.refresh_memory();
                let total = system.total_memory();
                let memory_fraction = if total == 0 {
                    0.0
                } else {
                    system.used_memory() as f32 / total as f32
                };
                cpu_metrics.store(system.global_cpu_usage().to_bits(), Ordering::Relaxed);
                memory_metrics.store(memory_fraction.to_bits(), Ordering::Relaxed);
                thread::park_timeout(Duration::from_secs(2));
            }
        });
        Self {
            key_times: Vec::new(),
            cpu_usage,
            memory_fraction,
            stop_metrics,
            metrics_worker: Some(metrics_worker),
        }
    }

    pub fn note_input(&mut self) {
        let now = Instant::now();
        self.key_times.push(now);
        self.key_times
            .retain(|time| now.duration_since(*time) <= Duration::from_secs(1));
    }

    pub fn snapshot(&mut self) -> Metrics {
        let now = Instant::now();
        self.key_times
            .retain(|time| now.duration_since(*time) <= Duration::from_secs(1));
        Metrics {
            input_rate: self.key_times.len() as f32,
            cpu_usage: f32::from_bits(self.cpu_usage.load(Ordering::Relaxed)),
            memory_fraction: f32::from_bits(self.memory_fraction.load(Ordering::Relaxed)),
        }
    }
}

impl Drop for Reactivity {
    fn drop(&mut self) {
        self.stop_metrics.store(true, Ordering::Release);
        if let Some(worker) = self.metrics_worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
