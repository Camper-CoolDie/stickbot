use bonfire::models::notification::ChatMessageCreated;
use bonfire::models::{AnyNotification, ChatMessage, ChatTag, Notification, Publication};

pub enum Event {
    ChatMessage(Box<Publication<ChatMessage>>),
    ChatRead((u64, u64)),
}
impl Event {
    pub fn new(notification: Notification) -> Option<Self> {
        match notification.spec {
            AnyNotification::ChatMessageCreated(ChatMessageCreated { message, .. }) => {
                matches!(message.spec.chat_tag, ChatTag::Direct { .. })
                    .then_some(Event::ChatMessage(Box::new(message)))
            }
            AnyNotification::ChatRead(ChatTag::Direct { ids }) => Some(Event::ChatRead(ids)),
            _ => None,
        }
    }
}
