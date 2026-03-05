use alloc::{collections::VecDeque, sync::Arc};

use axerrno::{AxError, AxResult, LinuxError};
use axpoll::PollSet;
use axsync::Mutex;

/// Receiver for Netlink messages.
#[derive(Clone)]
pub struct MessageReceiver<Message> {
    message_queue: Arc<Mutex<MessageQueue<Message>>>,
    poller: Arc<PollSet>,
}

/// Queue for Netlink messages.
pub(super) struct MessageQueue<Message> {
    messages: VecDeque<Message>,
    total_length: usize,
    error: Option<AxError>,
}

impl<Message> MessageQueue<Message> {
    /// Creates a new MessageQueue and its corresponding MessageReceiver.
    pub(super) fn new_pair(poller: Arc<PollSet>) -> (Arc<Mutex<Self>>, MessageReceiver<Message>) {
        let queue = Arc::new(Mutex::new(Self {
            messages: VecDeque::new(),
            total_length: 0,
            error: None,
        }));
        let receiver = MessageReceiver {
            message_queue: queue.clone(),
            poller: poller.clone(),
        };
        (queue, receiver)
    }

    /// Checks if the message queue is empty.
    pub(super) fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

/// Trait for messages that can be queued.
pub trait QueueableMessage {
    fn total_len(&self) -> usize;
}

impl<Message: QueueableMessage> MessageQueue<Message> {
    /// Dequeues a message if the provided function indicates to do so.
    pub(super) fn dequeue_if<F, R>(&mut self, f: F) -> AxResult<R>
    where
        F: FnOnce(&Message, usize) -> AxResult<(bool, R)>,
    {
        if let Some(error) = self.error.take() {
            return Err(error);
        }

        let Some(message) = self.messages.front() else {
            debug!("No message to dequeue");
            return Err(AxError::WouldBlock);
        };

        let length = message.total_len();
        let (should_pop, result) = f(message, length)?;
        if should_pop {
            self.messages.pop_front().unwrap();
            self.total_length -= length;
        }

        Ok(result)
    }

    /// Enqueues a new message into the queue.
    #[must_use]
    fn enqueue(&mut self, message: Message) -> bool {
        let length = message.total_len();

        if self.total_length.saturating_add(length) > crate::consts::NETLINK_DEFAULT_BUF_SIZE {
            self.error = Some(AxError::from(LinuxError::ENOBUFS));
            return false;
        }

        self.messages.push_back(message);
        self.total_length += length;

        true
    }
}

impl<Message: QueueableMessage> MessageReceiver<Message> {
    /// Enqueues a message into the receiver's message queue.
    pub(super) fn enqueue_message(&self, message: Message) {
        let is_ok = self.message_queue.lock().enqueue(message);
        if is_ok {
            trace!("Message enqueued successfully");
            self.poller.wake();
        } else {
            warn!("Failed to enqueue message");
        }
    }
}
