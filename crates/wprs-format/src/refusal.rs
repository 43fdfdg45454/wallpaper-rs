use std::fmt;
use std::ops::Deref;
use wprs_core::message::Message;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    text: String,
    said: Option<Box<Message>>,
}

impl Refusal {
    pub fn said(&self) -> Option<&Message> {
        self.said.as_deref()
    }

    pub fn put(&self, message: Message, at: usize) -> Message {
        match &self.said {
            Some(inner) => message.with(at, (**inner).clone()),
            None => message.texts(&[at]),
        }
    }
}

impl Deref for Refusal {
    type Target = str;
    fn deref(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl From<Message> for Refusal {
    fn from(said: Message) -> Refusal {
        Refusal { text: said.to_string(), said: Some(Box::new(said)) }
    }
}

impl From<String> for Refusal {
    fn from(text: String) -> Refusal {
        Refusal { text, said: None }
    }
}

impl From<&str> for Refusal {
    fn from(text: &str) -> Refusal {
        text.to_owned().into()
    }
}

impl From<Refusal> for String {
    fn from(reason: Refusal) -> String {
        reason.text
    }
}

impl PartialEq<&str> for Refusal {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}
