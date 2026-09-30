use std::sync::mpsc;

use super::*;

fn event(width: u16) -> TerminalEvent {
    TerminalEvent::Resize { width, height: 24 }
}

fn await_waiter(queue: &InputQueue) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !lock(&queue.state).waiting {
        assert!(Instant::now() < deadline, "consumer never began waiting");
        thread::yield_now();
    }
}

#[test]
fn events_are_fifo_and_zero_timeout_is_nonblocking() {
    let queue = InputQueue::new();
    assert_eq!(queue.poll(Duration::ZERO).unwrap(), EventPoll::Timeout);
    queue.push(event(80));
    queue.push(event(81));
    for width in [80, 81] {
        assert_eq!(
            queue.poll(Duration::ZERO).unwrap(),
            EventPoll::Event(event(width))
        );
    }
    assert_eq!(queue.poll(Duration::ZERO).unwrap(), EventPoll::Timeout);
}

#[test]
fn finite_timeout_waits_and_releases_the_wait_flag() {
    let queue = InputQueue::new();
    let timeout = Duration::from_millis(10);
    let start = Instant::now();
    assert_eq!(queue.poll(timeout).unwrap(), EventPoll::Timeout);
    assert!(start.elapsed() >= timeout);
    assert!(!lock(&queue.state).waiting);
    assert_eq!(queue.poll(Duration::ZERO).unwrap(), EventPoll::Timeout);
    assert!(matches!(
        queue.poll(Duration::MAX),
        Err(SessionError::InvalidTimeout)
    ));
    assert!(!lock(&queue.state).waiting);
}

#[test]
fn duplicate_wait_is_rejected_and_close_wakes_the_consumer() {
    let queue = Arc::new(InputQueue::new());
    let (send, receive) = mpsc::channel();
    let waiting = Arc::clone(&queue);
    let worker = thread::spawn(move || send.send(waiting.poll(Duration::from_secs(60))).unwrap());
    await_waiter(&queue);
    assert!(matches!(
        queue.poll(Duration::ZERO),
        Err(SessionError::ConcurrentEventWait)
    ));
    queue.begin_close();
    assert_eq!(
        receive
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap(),
        EventPoll::Closed
    );
    worker.join().unwrap();
    assert!(!lock(&queue.state).waiting);
    assert_eq!(queue.poll(Duration::ZERO).unwrap(), EventPoll::Closed);
}

#[test]
fn close_discards_queued_input_and_cancels_full_queue_backpressure() {
    let queue = Arc::new(InputQueue::new());
    for _ in 0..QUEUE_CAPACITY {
        queue.push(event(80));
    }
    let (started_send, started_receive) = mpsc::channel();
    let (done_send, done_receive) = mpsc::channel();
    let producer = Arc::clone(&queue);
    let worker = thread::spawn(move || {
        started_send.send(()).unwrap();
        done_send.send(producer.push(event(81))).unwrap();
    });
    started_receive
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    assert!(
        done_receive
            .recv_timeout(Duration::from_millis(10))
            .is_err()
    );
    queue.begin_close();
    assert!(!done_receive.recv_timeout(Duration::from_secs(2)).unwrap());
    worker.join().unwrap();
    assert_eq!(queue.poll(Duration::ZERO).unwrap(), EventPoll::Closed);
    assert!(lock(&queue.state).queue.is_empty());
}

#[test]
fn consuming_an_event_releases_a_blocked_producer() {
    let queue = Arc::new(InputQueue::new());
    for _ in 0..QUEUE_CAPACITY {
        queue.push(event(80));
    }
    let (send, receive) = mpsc::channel();
    let producer = Arc::clone(&queue);
    let worker = thread::spawn(move || send.send(producer.push(event(81))).unwrap());
    assert!(matches!(
        queue.poll(Duration::ZERO),
        Ok(EventPoll::Event(_))
    ));
    assert!(receive.recv_timeout(Duration::from_secs(2)).unwrap());
    worker.join().unwrap();
    assert_eq!(lock(&queue.state).queue.len(), QUEUE_CAPACITY);
}

struct BrokenSource {
    panic: bool,
}

impl InputSource for BrokenSource {
    fn next(&mut self, _: Duration) -> io::Result<Option<TerminalEvent>> {
        assert!(!self.panic, "injected input panic");
        Err(io::Error::other("injected reader failure"))
    }
}

#[test]
fn reader_errors_and_panics_are_sticky_and_do_not_poison_bookkeeping() {
    for panic in [false, true] {
        let queue = Arc::new(InputQueue::new());
        queue.push(event(80));
        start_reader(Arc::clone(&queue), BrokenSource { panic })
            .unwrap()
            .join()
            .unwrap();
        for _ in 0..2 {
            assert!(matches!(
                queue.poll(Duration::ZERO),
                Err(SessionError::Input(_))
            ));
            assert!(!lock(&queue.state).waiting);
        }
        assert!(!queue.state.is_poisoned());
        queue.begin_close();
        assert_eq!(queue.poll(Duration::ZERO).unwrap(), EventPoll::Closed);
    }
}

#[test]
fn reader_failure_wakes_an_existing_event_wait() {
    let queue = Arc::new(InputQueue::new());
    let consumer = Arc::clone(&queue);
    let (send, receive) = mpsc::channel();
    let worker = thread::spawn(move || send.send(consumer.poll(Duration::from_secs(60))).unwrap());
    await_waiter(&queue);
    queue.fail("read failed".into());
    assert!(matches!(
        receive.recv_timeout(Duration::from_secs(2)).unwrap(),
        Err(SessionError::Input(_))
    ));
    worker.join().unwrap();
}
