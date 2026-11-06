use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use wdys::audio::AudioClip;
use wdys::error::TranscribeError;
use wdys::transcribe::{LazyModel, ModelLoader, SpeechModel};

struct FakeModel;

impl SpeechModel for FakeModel {
    fn transcribe(&mut self, _clip: &AudioClip) -> Result<String, TranscribeError> {
        Ok("hello".to_string())
    }
}

struct CountingLoader {
    loads: Rc<Cell<u32>>,
    fail_next: Rc<Cell<bool>>,
}

impl ModelLoader for CountingLoader {
    fn load(&self, _path: &Path) -> Result<Box<dyn SpeechModel>, TranscribeError> {
        self.loads.set(self.loads.get() + 1);
        if self.fail_next.get() {
            return Err(TranscribeError::Load("bad model".into()));
        }
        Ok(Box::new(FakeModel))
    }
}

const IDLE: u64 = 1000;

fn lazy() -> (LazyModel, Rc<Cell<u32>>, Rc<Cell<bool>>) {
    let loads = Rc::new(Cell::new(0));
    let fail_next = Rc::new(Cell::new(false));
    let loader = CountingLoader { loads: loads.clone(), fail_next: fail_next.clone() };
    (LazyModel::new(Box::new(loader), PathBuf::from("m.bin"), IDLE), loads, fail_next)
}

fn clip() -> AudioClip {
    AudioClip { samples: vec![0.0; 16_000] }
}

#[test]
fn loads_on_first_use_only() {
    let (mut model, loads, _) = lazy();
    assert!(!model.is_loaded());
    assert_eq!(model.transcribe(&clip(), 0).unwrap(), "hello");
    model.transcribe(&clip(), 10).unwrap();
    assert_eq!(loads.get(), 1);
    assert!(model.is_loaded());
}

#[test]
fn unloads_after_idle_timeout() {
    let (mut model, _, _) = lazy();
    model.transcribe(&clip(), 100).unwrap();
    assert!(model.unload_if_idle(100 + IDLE));
    assert!(!model.is_loaded());
}

#[test]
fn does_not_unload_before_timeout() {
    let (mut model, _, _) = lazy();
    model.transcribe(&clip(), 100).unwrap();
    assert!(!model.unload_if_idle(100 + IDLE - 1));
    assert!(model.is_loaded());
}

#[test]
fn use_refreshes_the_idle_timer() {
    let (mut model, _, _) = lazy();
    model.transcribe(&clip(), 100).unwrap();
    model.transcribe(&clip(), 900).unwrap();
    assert!(!model.unload_if_idle(1100));
}

#[test]
fn reloads_after_unload() {
    let (mut model, loads, _) = lazy();
    model.transcribe(&clip(), 0).unwrap();
    model.unload_if_idle(IDLE);
    model.transcribe(&clip(), IDLE + 1).unwrap();
    assert_eq!(loads.get(), 2);
}

#[test]
fn load_failure_surfaces_as_error_and_retries_next_time() {
    let (mut model, loads, fail_next) = lazy();
    fail_next.set(true);
    assert!(matches!(model.transcribe(&clip(), 0), Err(TranscribeError::Load(_))));
    assert!(!model.is_loaded());
    fail_next.set(false);
    assert_eq!(model.transcribe(&clip(), 5).unwrap(), "hello");
    assert_eq!(loads.get(), 2);
}

#[test]
fn unload_when_nothing_loaded_is_false() {
    let (mut model, _, _) = lazy();
    assert!(!model.unload_if_idle(10 * IDLE));
}
