
use std::time::Duration;

use lyre_core::{
    gst::GstBackend,
    player::{AudioBackend, BackendError, BackendEvent},
    NullBackend,
};

pub enum Backend {
    Gst(Box<GstBackend>),
    Null(NullBackend),
}

impl Backend {

    pub fn detect() -> Backend {
        match GstBackend::new() {
            Ok(backend) => Backend::Gst(Box::new(backend)),
            Err(e) => {
                eprintln!("warning: no audio backend available ({e}); starting in silent mode");
                Backend::Null(NullBackend::new())
            }
        }
    }

    pub fn null() -> Backend {
        Backend::Null(NullBackend::new())
    }

    pub fn is_silent(&self) -> bool {
        matches!(self, Backend::Null(_))
    }
}

macro_rules! dispatch {
    ($self:ident, $inner:ident => $call:expr) => {
        match $self {
            Backend::Gst($inner) => $call,
            Backend::Null($inner) => $call,
        }
    };
}

impl AudioBackend for Backend {
    fn play_uri(&mut self, uri: &str) -> Result<(), BackendError> {
        dispatch!(self, b => b.play_uri(uri))
    }
    fn pause(&mut self) -> Result<(), BackendError> {
        dispatch!(self, b => b.pause())
    }
    fn resume(&mut self) -> Result<(), BackendError> {
        dispatch!(self, b => b.resume())
    }
    fn stop(&mut self) -> Result<(), BackendError> {
        dispatch!(self, b => b.stop())
    }
    fn set_volume(&mut self, volume: f64) {
        dispatch!(self, b => b.set_volume(volume))
    }
    fn volume(&self) -> f64 {
        dispatch!(self, b => b.volume())
    }
    fn position(&self) -> Option<Duration> {
        dispatch!(self, b => b.position())
    }
    fn duration(&self) -> Option<Duration> {
        dispatch!(self, b => b.duration())
    }
    fn seek(&mut self, position: Duration) -> Result<(), BackendError> {
        dispatch!(self, b => b.seek(position))
    }
    fn poll_events(&mut self) -> Vec<BackendEvent> {
        dispatch!(self, b => b.poll_events())
    }
}
