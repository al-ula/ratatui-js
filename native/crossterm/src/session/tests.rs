use std::{sync::mpsc, thread, time::Instant};

use ratatui::backend::TestBackend;
use ratatui_js_core::ErrorCode;

use super::*;
use crate::terminal::{
    Ownership,
    tests::{MockControl, MockState},
};

const FRAME: &[u8] =
    br#"{"protocolVersion":1,"root":{"type":"paragraph","lines":[[{"text":"hello"}]]}}"#;

fn fixture<R: RenderTarget>(
    renderer: R,
) -> (
    SessionInner<R>,
    Arc<Mutex<MockState>>,
    Arc<Mutex<Ownership>>,
) {
    let control = Arc::new(Mutex::new(MockState::default()));
    let slot = Arc::new(Mutex::new(Ownership::Free));
    let session = SessionInner::open(
        SessionOptions::default(),
        Box::new(MockControl(Arc::clone(&control))),
        OwnerGuard::acquire(Arc::clone(&slot)).unwrap(),
        || Ok(renderer),
        |_| thread::Builder::new().spawn(|| {}),
    )
    .unwrap();
    (session, control, slot)
}

#[test]
fn rejected_frames_preserve_rendering_and_close_rejects_future_renders() {
    let (session, _, slot) = fixture(Renderer::new(TestBackend::new(12, 3)).unwrap());
    let error = session.render_json(b"{").unwrap_err();
    match error {
        SessionError::Frame(error) => assert_eq!(error.code(), ErrorCode::InvalidJson),
        other => panic!("unexpected error: {other}"),
    }
    let result = session.render_json(FRAME).unwrap();
    assert_eq!((result.width, result.height), (12, 3));
    session.close().unwrap();
    assert_eq!(*lock(&slot), Ownership::Free);
    assert!(matches!(
        session.render_json(FRAME),
        Err(SessionError::Closed)
    ));
    assert_eq!(
        session.input.poll(Duration::ZERO).unwrap(),
        EventPoll::Closed
    );
}

#[test]
fn repeated_and_concurrent_close_restore_once_and_preserve_the_error_outcome() {
    let (session, control, slot) = fixture(Renderer::new(TestBackend::new(12, 3)).unwrap());
    lock(&control).failures = vec!["show cursor", "disable raw mode"];
    let session = Arc::new(session);
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let session = Arc::clone(&session);
            thread::spawn(move || session.close())
        })
        .collect();
    for worker in workers {
        match worker.join().unwrap() {
            Err(SessionError::Shutdown(failures)) => assert_eq!(failures.len(), 2),
            other => panic!("unexpected close outcome: {other:?}"),
        }
    }
    assert!(session.close().is_err());
    assert_eq!(
        lock(&control)
            .calls
            .iter()
            .filter(|&&step| step == "disable raw mode")
            .count(),
        1
    );
    assert_eq!(*lock(&slot), Ownership::Poisoned);
}

#[test]
fn renderer_and_reader_creation_failures_roll_back_modes() {
    for reader_failure in [false, true] {
        let control = Arc::new(Mutex::new(MockState::default()));
        let slot = Arc::new(Mutex::new(Ownership::Free));
        let result = SessionInner::open(
            SessionOptions::default(),
            Box::new(MockControl(Arc::clone(&control))),
            OwnerGuard::acquire(Arc::clone(&slot)).unwrap(),
            || {
                if reader_failure {
                    Ok(Renderer::new(TestBackend::new(12, 3)).unwrap())
                } else {
                    Err(SessionError::io("create renderer", "injected failure"))
                }
            },
            |_| Err(io::Error::other("injected thread creation failure")),
        );
        assert!(result.is_err());
        assert!(lock(&control).calls.contains(&"disable raw mode"));
        assert!(lock(&control).calls.contains(&"leave alternate screen"));
        assert_eq!(*lock(&slot), Ownership::Free);
    }
}

#[test]
fn initialization_reports_original_and_rollback_errors() {
    let control = Arc::new(Mutex::new(MockState {
        failures: vec!["hide cursor", "disable raw mode"],
        ..MockState::default()
    }));
    let slot = Arc::new(Mutex::new(Ownership::Free));
    let result = SessionInner::open(
        SessionOptions::default(),
        Box::new(MockControl(control)),
        OwnerGuard::acquire(Arc::clone(&slot)).unwrap(),
        || {
            Renderer::new(TestBackend::new(12, 3))
                .map_err(|e| SessionError::io("create renderer", e))
        },
        |_| thread::Builder::new().spawn(|| {}),
    );
    match result {
        Err(SessionError::Initialization { cause, cleanup }) => {
            assert!(matches!(
                *cause,
                SessionError::Io {
                    operation: "hide cursor",
                    ..
                }
            ));
            assert_eq!(cleanup.len(), 1);
            assert_eq!(cleanup[0].operation, "disable raw mode");
        }
        _ => panic!("expected initialization and rollback errors"),
    }
    assert_eq!(*lock(&slot), Ownership::Poisoned);
}

struct FailingRenderer {
    panic: bool,
}

impl RenderTarget for FailingRenderer {
    fn render_json(&mut self, _: &[u8]) -> Result<RenderResult, RenderError> {
        assert!(!self.panic, "injected render panic");
        Err(RenderError::Backend("injected output failure".into()))
    }
}

#[test]
fn backend_failures_and_panics_disable_rendering_but_not_cleanup() {
    for panic in [false, true] {
        let (session, _, slot) = fixture(FailingRenderer { panic });
        assert!(session.render_json(FRAME).is_err());
        assert!(matches!(
            session.render_json(FRAME),
            Err(SessionError::RenderingFailed)
        ));
        assert!(!session.renderer.is_poisoned());
        session.close().unwrap();
        assert_eq!(*lock(&slot), Ownership::Free);
    }
}

struct BlockingRenderer {
    entered: mpsc::Sender<()>,
    resume: mpsc::Receiver<()>,
}

impl RenderTarget for BlockingRenderer {
    fn render_json(&mut self, _: &[u8]) -> Result<RenderResult, RenderError> {
        self.entered.send(()).unwrap();
        self.resume.recv_timeout(Duration::from_secs(5)).unwrap();
        Ok(RenderResult::default())
    }
}

#[test]
fn close_finishes_admitted_render_but_rejects_queued_render() {
    let (entered_send, entered_receive) = mpsc::channel();
    let (resume_send, resume_receive) = mpsc::channel();
    let (session, _, _) = fixture(BlockingRenderer {
        entered: entered_send,
        resume: resume_receive,
    });
    let session = Arc::new(session);
    let render_session = Arc::clone(&session);
    let render = thread::spawn(move || render_session.render_json(FRAME));
    entered_receive
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    let queued_session = Arc::clone(&session);
    let queued = thread::spawn(move || queued_session.render_json(FRAME));
    let close_session = Arc::clone(&session);
    let (close_send, close_receive) = mpsc::channel();
    let close = thread::spawn(move || close_send.send(close_session.close()).unwrap());
    let deadline = Instant::now() + Duration::from_secs(2);
    while lock(&session.input.state).lifecycle != Lifecycle::Closing {
        assert!(Instant::now() < deadline, "close never began");
        thread::yield_now();
    }
    assert!(close_receive.try_recv().is_err());
    assert_eq!(
        session.input.poll(Duration::ZERO).unwrap(),
        EventPoll::Closed
    );
    resume_send.send(()).unwrap();
    render.join().unwrap().unwrap();
    assert!(matches!(queued.join().unwrap(), Err(SessionError::Closed)));
    close_receive
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    close.join().unwrap();
}

#[test]
fn drop_is_a_cleanup_fallback() {
    let (session, control, slot) = fixture(Renderer::new(TestBackend::new(12, 3)).unwrap());
    drop(session);
    assert!(lock(&control).calls.contains(&"disable raw mode"));
    assert_eq!(*lock(&slot), Ownership::Free);
}

#[test]
fn rendering_does_not_wait_for_input_and_close_wakes_the_poller() {
    let (session, _, _) = fixture(Renderer::new(TestBackend::new(12, 3)).unwrap());
    let session = Arc::new(session);
    let consumer = Arc::clone(&session);
    let (send, receive) = mpsc::channel();
    let worker = thread::spawn(move || {
        send.send(consumer.input.poll(Duration::from_secs(60)))
            .unwrap()
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    while !matches!(
        session.input.poll(Duration::ZERO),
        Err(SessionError::ConcurrentEventWait)
    ) {
        assert!(Instant::now() < deadline, "event wait never began");
        thread::yield_now();
    }
    session.render_json(FRAME).unwrap();
    session.close().unwrap();
    assert_eq!(
        receive
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap(),
        EventPoll::Closed
    );
    worker.join().unwrap();
}

#[test]
fn poisoned_renderer_is_not_reused_but_can_still_be_closed() {
    let (session, _, slot) = fixture(Renderer::new(TestBackend::new(12, 3)).unwrap());
    let session = Arc::new(session);
    let poison_session = Arc::clone(&session);
    assert!(
        thread::spawn(move || {
            let _renderer = poison_session.renderer.lock().unwrap();
            panic!("injected lock poisoning");
        })
        .join()
        .is_err()
    );
    assert!(matches!(
        session.render_json(FRAME),
        Err(SessionError::RenderingFailed)
    ));
    session.close().unwrap();
    assert_eq!(*lock(&slot), Ownership::Free);
}

#[test]
fn initialization_panic_rolls_back_attempted_modes() {
    let control = Arc::new(Mutex::new(MockState {
        panics: vec!["hide cursor"],
        ..MockState::default()
    }));
    let slot = Arc::new(Mutex::new(Ownership::Free));
    let result = SessionInner::open(
        SessionOptions::default(),
        Box::new(MockControl(Arc::clone(&control))),
        OwnerGuard::acquire(Arc::clone(&slot)).unwrap(),
        || {
            Renderer::new(TestBackend::new(12, 3))
                .map_err(|e| SessionError::io("create renderer", e))
        },
        |_| thread::Builder::new().spawn(|| {}),
    );
    assert!(matches!(
        result,
        Err(SessionError::Panic("session initialization"))
    ));
    assert!(lock(&control).calls.contains(&"show cursor"));
    assert!(lock(&control).calls.contains(&"disable raw mode"));
    assert_eq!(*lock(&slot), Ownership::Free);
}
