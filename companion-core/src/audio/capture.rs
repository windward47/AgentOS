//! Microphone capture via `cpal`.
//!
//! [`AudioCapture`] opens the default input stream and writes samples into a
//! small ring buffer (default 2 seconds) for VAD and ASR consumption.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use crate::audio::AudioError;

/// Ring buffer of mono f32 samples.
pub struct RingBuffer {
    buffer: Vec<f32>,
    capacity: usize,
    write_pos: usize,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0.0; capacity],
            capacity,
            write_pos: 0,
        }
    }

    /// Write a chunk of samples, wrapping around.
    pub fn write(&mut self, samples: &[f32]) {
        for &s in samples {
            self.buffer[self.write_pos] = s;
            self.write_pos = (self.write_pos + 1) % self.capacity;
        }
    }

    /// Read the most recent `count` samples (linear, newest at end).
    pub fn read_recent(&self, count: usize) -> Vec<f32> {
        let count = count.min(self.capacity);
        let mut result = Vec::with_capacity(count);
        // Start from `write_pos - count` (wrapping)
        let start = (self.write_pos + self.capacity - count) % self.capacity;
        for i in 0..count {
            result.push(self.buffer[(start + i) % self.capacity]);
        }
        result
    }

    /// Clear the buffer (zero fill).
    pub fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
    }
}

/// Audio capture state shared between the stream callback and consumers.
pub struct CaptureInner {
    pub buffer: RingBuffer,
    pub sample_rate: u32,
}

/// Microphone capture controller.
pub struct AudioCapture {
    stream: Option<cpal::Stream>,
    inner: Arc<Mutex<CaptureInner>>,
    running: Arc<AtomicBool>,
}

impl AudioCapture {
    /// Open the default input device and start capturing.
    pub fn start(sample_rate: u32) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host.default_input_device()
            .ok_or_else(|| AudioError::DeviceNotFound("no microphone found".into()))?;

        let config = cpal::StreamConfig {
            channels: 1,                     // mono
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let buffer_size = (sample_rate as usize) * 2; // 2 seconds ring
        let inner = Arc::new(Mutex::new(CaptureInner {
            buffer: RingBuffer::new(buffer_size),
            sample_rate,
        }));

        let running = Arc::new(AtomicBool::new(true));
        let inner_clone = inner.clone();
        let running_clone = running.clone();

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if running_clone.load(Ordering::Relaxed) {
                    if let Ok(mut guard) = inner_clone.lock() {
                        guard.buffer.write(data);
                    }
                }
            },
            move |err| {
                eprintln!("[AudioCapture] stream error: {err}");
            },
            None, // timeout
        ).map_err(|e| AudioError::StreamError(e.to_string()))?;

        stream.play().map_err(|e| AudioError::StreamError(e.to_string()))?;

        Ok(Self {
            stream: Some(stream),
            inner,
            running,
        })
    }

    /// Stop the capture stream.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(s) = self.stream.take() {
            s.pause().ok();
            drop(s);
        }
    }

    /// Read the most recent audio chunk (duration in ms).
    pub fn read_chunk(&self, duration_ms: u32) -> Vec<f32> {
        let guard = self.inner.lock().unwrap();
        let sample_count = (guard.sample_rate as u64 * duration_ms as u64 / 1000) as usize;
        guard.buffer.read_recent(sample_count)
    }

    /// Get the sample rate.
    pub fn sample_rate(&self) -> u32 {
        self.inner.lock().unwrap().sample_rate
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.stop();
    }
}
