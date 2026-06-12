//! Global voice capture manager — accumulates ALL samples (Vec, no ring buffer).
//!
//! Uses cpal directly (like vox) to collect i16 samples from start to stop,
//! then converts to f32 for Companion's ASR providers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam::channel::{self, Sender};

/// Commands sent to the capture thread.
enum CaptureCommand {
    Start(Sender<bool>),
    Stop(Sender<Vec<i16>>),
}

/// Handle to the capture thread.
pub struct CaptureHandle {
    cmd_tx: Sender<CaptureCommand>,
}

impl CaptureHandle {
    /// Start recording. Blocks until the mic stream is confirmed active.
    pub fn start(&self) -> bool {
        let (tx, rx) = channel::bounded(1);
        let _ = self.cmd_tx.send(CaptureCommand::Start(tx));
        rx.recv().unwrap_or(false)
    }

    /// Stop recording and return captured PCM i16 samples (16kHz mono).
    pub fn stop(&self) -> Vec<i16> {
        let (tx, rx) = channel::bounded(1);
        let _ = self.cmd_tx.send(CaptureCommand::Stop(tx));
        rx.recv().unwrap_or_default()
    }
}

/// Convert i16 samples to f32 (scaled to -1..1).
pub fn i16_to_f32(samples: &[i16]) -> Vec<f32> {
    samples.iter().map(|&s| s as f32 / i16::MAX as f32).collect()
}

/// Spawn the capture thread. Returns a handle for start/stop.
pub fn spawn_capture_manager() -> CaptureHandle {
    let (cmd_tx, cmd_rx) = channel::unbounded::<CaptureCommand>();

    std::thread::spawn(move || {
        // Shared state between the capture thread and the stream callback
        let samples: Arc<Mutex<Vec<i16>>> = Arc::new(Mutex::new(Vec::new()));
        let stop_flag: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let mut stream: Option<cpal::Stream> = None;

        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                CaptureCommand::Start(reply) => {
                    // Stop any existing stream first
                    drop(stream.take());
                    stop_flag.store(false, Ordering::Relaxed);
                    samples.lock().unwrap().clear();

                    let host = cpal::default_host();
                    let device = match host.default_input_device() {
                        Some(d) => d,
                        None => {
                            log::error!("[CaptureManager] No default input device");
                            let _ = reply.send(false);
                            continue;
                        }
                    };

                    let config = match device.default_input_config() {
                        Ok(c) => c,
                        Err(e) => {
                            log::error!("[CaptureManager] Failed to get input config: {}", e);
                            let _ = reply.send(false);
                            continue;
                        }
                    };

                    log::info!(
                        "[CaptureManager] Device: {}, config: {:?}",
                        device.name().unwrap_or_default(),
                        config
                    );

                    let stop = stop_flag.clone();
                    let buf = samples.clone();
                    let channels = config.channels() as usize;

                    let result = match config.sample_format() {
                        cpal::SampleFormat::I16 => {
                            device.build_input_stream(
                                &config.config(),
                                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                                    if stop.load(Ordering::Relaxed) {
                                        return;
                                    }
                                    let mut lock = buf.lock().unwrap();
                                    if channels > 1 {
                                        lock.reserve(data.len() / channels);
                                        for (&sample, i) in data.iter().zip(0..) {
                                            if i % channels == 0 {
                                                lock.push(sample);
                                            }
                                        }
                                    } else {
                                        lock.extend_from_slice(data);
                                    }
                                },
                                |err| log::error!("[CaptureManager] stream error: {}", err),
                                None,
                            )
                        }
                        cpal::SampleFormat::F32 => {
                            device.build_input_stream(
                                &config.config(),
                                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                                    if stop.load(Ordering::Relaxed) {
                                        return;
                                    }
                                    let mut lock = buf.lock().unwrap();
                                    if channels > 1 {
                                        lock.reserve(data.len() / channels);
                                        for (&sample, i) in data.iter().zip(0..) {
                                            if i % channels == 0 {
                                                lock.push((sample * i16::MAX as f32) as i16);
                                            }
                                        }
                                    } else {
                                        lock.reserve(data.len());
                                        for &sample in data {
                                            lock.push((sample * i16::MAX as f32) as i16);
                                        }
                                    }
                                },
                                |err| log::error!("[CaptureManager] stream error: {}", err),
                                None,
                            )
                        }
                        fmt => {
                            log::error!("[CaptureManager] Unsupported sample format: {:?}", fmt);
                            let _ = reply.send(false);
                            continue;
                        }
                    };

                    match result {
                        Ok(s) => {
                            if let Err(e) = s.play() {
                                log::error!("[CaptureManager] Failed to play stream: {}", e);
                                let _ = reply.send(false);
                                continue;
                            }
                            stream = Some(s);
                            log::info!("[CaptureManager] Recording started ✓");
                            let _ = reply.send(true);
                        }
                        Err(e) => {
                            log::error!("[CaptureManager] Failed to build stream: {}", e);
                            let _ = reply.send(false);
                        }
                    }
                }

                CaptureCommand::Stop(reply) => {
                    log::info!("[CaptureManager] Stopping recording...");
                    stop_flag.store(true, Ordering::Relaxed);

                    // Drop stream to stop capture (blocks until callback exits)
                    drop(stream.take());

                    // Small delay for the last callback to finish writing
                    std::thread::sleep(std::time::Duration::from_millis(50));

                    let captured = {
                        let lock = samples.lock().unwrap();
                        lock.clone()
                    };

                    log::info!(
                        "[CaptureManager] Captured {} samples ({:.1}s)",
                        captured.len(),
                        captured.len() as f64 / 16000.0
                    );

                    let _ = reply.send(captured);
                    stop_flag.store(false, Ordering::Relaxed);
                }
            }
        }
    });

    CaptureHandle { cmd_tx }
}
