//! Companion Core — pure-logic library crate.
//!
//! Contains all the module definitions, trait abstractions, and implementations.
//! Does NOT depend on Tauri — this crate can be used standalone or in tests.

pub mod agent;
pub mod asr;
pub mod audio;
pub mod config;
pub mod capture_mgr;
pub mod downloader;
pub mod hotkey;
pub mod inject;
pub mod mcp;
pub mod permissions;
pub mod sandbox;
pub mod tools;
pub mod tts;
