use wprs_core::dbus::error;
use zbus::fdo;
use zbus::message::{Header, Message};
use zbus::names::ErrorName;

#[derive(Debug)]
pub enum Error {
    Fdo(fdo::Error),
    Named(&'static str, String),
    Cancelled,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub fn invalid(message: impl Into<String>) -> Error {
    Error::Fdo(fdo::Error::InvalidArgs(message.into()))
}

pub fn failed(message: impl Into<String>) -> Error {
    Error::Fdo(fdo::Error::Failed(message.into()))
}

pub fn not_found(message: impl Into<String>) -> Error {
    Error::Named(error::NOT_FOUND, message.into())
}

impl Error {
    pub fn parts(&self) -> (String, String) {
        match self {
            Error::Fdo(e) => (
                zbus::DBusError::name(e).to_string(),
                zbus::DBusError::description(e).unwrap_or_default().into(),
            ),
            Error::Named(name, message) => ((*name).into(), message.clone()),
            Error::Cancelled => (String::new(), "cancelled".into()),
        }
    }
}

impl zbus::DBusError for Error {
    fn create_reply(&self, call: &Header<'_>) -> zbus::Result<Message> {
        let (name, message) = self.parts();
        let name = if name.is_empty() { "org.freedesktop.DBus.Error.Failed".into() } else { name };
        Message::error(call, name).build(&(message,))
    }

    fn name(&self) -> ErrorName<'_> {
        match self {
            Error::Fdo(e) => zbus::DBusError::name(e),
            Error::Named(name, _) => ErrorName::from_static_str_unchecked(name),
            Error::Cancelled => ErrorName::from_static_str_unchecked("org.freedesktop.DBus.Error.Failed"),
        }
    }

    fn description(&self) -> Option<&str> {
        match self {
            Error::Fdo(e) => e.description(),
            Error::Named(_, message) => Some(message),
            Error::Cancelled => Some("cancelled"),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.parts().1)
    }
}

impl std::error::Error for Error {}

impl From<fdo::Error> for Error {
    fn from(e: fdo::Error) -> Self {
        Error::Fdo(e)
    }
}

impl From<wprs_library::Error> for Error {
    fn from(e: wprs_library::Error) -> Self {
        use wprs_library::Error as L;
        match e {
            L::NotFound(..) => not_found(e.to_string()),
            L::Exists(..) => Error::Named(error::EXISTS, e.to_string()),
            L::Invalid(..) | L::Query(..) => invalid(e.to_string()),
            L::Conflict => Error::Named(error::CONFLICT, e.to_string()),
            L::Cancelled => Error::Cancelled,
            e => failed(e.to_string()),
        }
    }
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        let e = match e.downcast::<Error>() {
            Ok(e) => return e,
            Err(e) => e,
        };
        let e = match e.downcast::<fdo::Error>() {
            Ok(e) => return Error::Fdo(e),
            Err(e) => e,
        };
        match e.downcast::<wprs_library::Error>() {
            Ok(e) => e.into(),
            Err(e) => failed(format!("{e:#}")),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        failed(e.to_string())
    }
}

impl From<wprs_sync::Error> for Error {
    fn from(e: wprs_sync::Error) -> Self {
        match e {
            wprs_sync::Error::Invalid(message) => invalid(message),
            e => failed(e.to_string()),
        }
    }
}
