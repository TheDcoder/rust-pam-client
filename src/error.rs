//! Error structs and related helpers
#[doc(no_inline)]
pub use pam_sys::types::PamReturnCode as ReturnCode;
use pam_sys::types::{PamHandle};
use pam_sys::wrapped::strerror;

use std::any::type_name;
use std::convert::TryFrom;
use std::cmp::{Eq, PartialEq};
use std::error;
use std::hash::{Hash, Hasher};
use std::fmt::{Display, Formatter, Debug, Result as FmtResult};
use std::marker::PhantomData;
use std::io;

/// The error payload type for errors that never have payloads.
///
/// Like `std::convert::Infallible` but with a less confusing name, given
/// the context it's used in here. Might become a type alias to `!` when
/// the [`!` never type](https://doc.rust-lang.org/std/primitive.never.html)
/// is stabilized.
#[derive(Copy, Clone, Debug)]
pub enum NoPayload {}

impl Display for NoPayload {
	fn fmt(&self, _: &mut Formatter<'_>) -> FmtResult { match *self {} }
}

impl PartialEq for NoPayload {
	fn eq(&self, _: &NoPayload) -> bool { match *self {} }
}

impl Eq for NoPayload {}

impl Hash for NoPayload {
	fn hash<H: Hasher>(&self, _: &mut H) { match *self {} }
}

/// Helper to implement `Debug` on `ErrorWith` with `T` not implementing `Debug`
enum DisplayHelper<T> { Some(PhantomData<T>), None }

impl<T> DisplayHelper<T> {
	#[inline]
	fn new(option: &Option<T>) -> Self {
		match option {
			None => Self::None,
			Some(_) => Self::Some(PhantomData)
		}
	}
}

impl<T> Debug for DisplayHelper<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match *self {
			Self::None => write!(f, "None"),
			Self::Some(_) => write!(f, "<{}>", type_name::<T>())
		}
	}
}

/// Base error type for PAM operations (possibly with a payload)
///
/// Errors originate from the PAM library, PAM modules or helper structs
/// in this crate. Currently no custom instances are supported.
#[must_use]
#[derive(Clone)]
pub struct ErrorWith<T> {
	code: ReturnCode,
	msg: String,
	payload: Option<T>
}

impl<T> ErrorWith<T> {
	/// Creates a new [`Error`] that takes a payload.
	///
	/// Functions that consume a struct can use the payload to transfer back
	/// ownership in error cases.
	pub fn with_payload(handle: &mut PamHandle, code: ReturnCode, payload: Option<T>) -> ErrorWith<T> {
		assert_ne!(code, ReturnCode::SUCCESS);
		Self {
			code,
			msg: match strerror(handle, code) {
				None => String::new(),
				Some(s) => s.into()
			},
			payload
		}
	}

	/// The error code.
	pub const fn code(&self) -> ReturnCode {
		self.code
	}

	/// Text representation of the error code, if available.
	pub fn message(&self) -> Option<&str> {
		if self.msg.is_empty() { None } else { Some(&self.msg) }
	}

	/// Returns a reference to an optional payload.
	pub fn payload(&self) -> Option<&T> {
		self.payload.as_ref()
	}

	/// Takes the payload out of the error message.
	///
	/// If a payload exists in this error, it will be moved into the returned
	/// [`Option`]. All further calls to [`payload()`][`Self::payload()`] and
	/// [`take_payload()`][`Self::take_payload()`] will return [`None`].
	pub fn take_payload(&mut self) -> Option<T> {
		match self.payload {
			Some(_) => self.payload.take(),
			None => None
		}
	}

	/// Drops any payload off the error message, if one exists.
	#[inline]
	pub fn drop_payload(self) -> Error {
		Error {
			code: self.code,
			msg: self.msg,
			payload: None
		}
	}

	pub fn map<U>(self, func: impl FnOnce(T) -> U) -> ErrorWith<U> {
		ErrorWith::<U> {
			code: self.code,
			msg: self.msg,
			payload: match self.payload {
				None => None,
				Some(object) => Some(func(object))
			}
		}
	}
}

impl<T> Debug for ErrorWith<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		// Hacky and not always correct, but the best we can do for now
		// without specialization
		if type_name::<T>() == type_name::<NoPayload>() {
			f.debug_struct("pam_client::Error")
			.field("code", &self.code)
			.field("msg", &self.msg)
			.finish()
		} else {
			f.debug_struct("pam_client::ErrorWith")
				.field("code", &self.code)
				.field("msg", &self.msg)
				.field("payload", &DisplayHelper::new(&self.payload))
				.finish()
		}
	}
}

/// Error type for PAM operations without error payload.
///
/// This variant never contains a payload.
#[doc(alias = "PamError")]
pub type Error = ErrorWith<NoPayload>;

impl Error {
	/// Creates a new [`Error`].
	pub fn new(handle: &mut PamHandle, code: ReturnCode) -> Error {
		assert_ne!(code, ReturnCode::SUCCESS);
		Error {
			code,
			msg: match strerror(handle, code) {
				None => String::new(),
				Some(s) => s.into()
			},
			payload: None
		}
	}

	/// Adds the payload to the error message and returns a corresponding
	/// [`ErrorWith<T>`] instance.
	pub fn into_with_payload<T>(self, payload: T) -> ErrorWith<T> {
		ErrorWith::<T> {
			code: self.code,
			msg: self.msg,
			payload: Some(payload)
		}
	}

	/// Converts the error message into a [`ErrorWith<T>`] instance without
	/// a payload.
	pub fn into<T>(self) -> ErrorWith<T> {
		ErrorWith::<T> {
			code: self.code,
			msg: self.msg,
			payload: None
		}
	}
}

impl<T> Display for ErrorWith<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		if self.msg.is_empty() {
			write!(f, "<{}>", self.code as i32)
		} else {
			f.write_str(&self.msg)
		}
	}
}

impl<T> error::Error for ErrorWith<T> {}

impl<T> PartialEq for ErrorWith<T> where T: PartialEq {
	fn eq(&self, other: &Self) -> bool {
		self.code == other.code && self.payload == other.payload
	}
}

impl<T> Eq for ErrorWith<T> where T:  Eq {}

impl<T> Hash for ErrorWith<T> where T: Hash {
	fn hash<H: Hasher>(&self, state: &mut H) {
		(self.code as i32).hash(state);
		self.payload.hash(state);
	}
}

/// Wrapping of a [`ReturnCode`] in a [`Error`] without a PAM context.
///
/// This is used internally to construct [`Error`] instances when no PAM
/// context is available. These instances won't have a message string, only
/// a code.
///
/// The conversion only fails on [`ReturnCode::SUCCESS`].
///
/// Examples:
/// ```rust
/// use std::convert::{TryFrom};
/// # use pam_client::{Error, ReturnCode};
///
/// let error = Error::try_from(ReturnCode::ABORT).unwrap();
/// println!("{:?}", error);
/// ```
/// ```rust
/// use std::convert::{TryInto};
/// # use pam_client::{Error, ReturnCode};
///
/// let error: Error = ReturnCode::ABORT.try_into().unwrap();
/// println!("{:?}", error);
/// ```
/// ```rust,should_panic
/// use std::convert::{TryInto};
/// # use pam_client::{Error, ReturnCode};
///
/// let error: Error = ReturnCode::SUCCESS.try_into().unwrap(); // should panic
/// ```
impl TryFrom<ReturnCode> for Error {
	type Error = ();
	fn try_from(code: ReturnCode) -> Result<Self, ()> {
		if code == ReturnCode::SUCCESS {
			Err(())
		} else {
			Ok(Error { code, msg: String::new(), payload: None })
		}
	}
}

/// Automatic wrapping in [`std::io::Error`] (if payload type is compatible).
///
/// ```rust
/// # use std::convert::TryInto;
/// # use pam_client::{Result, Error, ReturnCode};
/// # fn some_succeeding_pam_function() -> Result<()> { Ok(()) }
/// fn main() -> std::result::Result<(), std::io::Error> {
///     some_succeeding_pam_function()?;
///     Ok(())
/// }
/// ```
/// ```rust,should_panic
/// # use std::convert::{Infallible, TryInto};
/// # use pam_client::{Result, Error, ReturnCode};
/// # fn some_failing_pam_function() -> Result<Infallible> {
/// #     Err(ReturnCode::ABORT.try_into().unwrap())
/// # }
/// fn main() -> std::result::Result<(), std::io::Error> {
///     some_failing_pam_function()?;
///     Ok(())
/// }
/// ```
impl<T: Send + Sync + Debug + 'static> From<ErrorWith<T>> for io::Error {
	fn from(error: ErrorWith<T>) -> Self {
		io::Error::new(match error.code {
			ReturnCode::INCOMPLETE | ReturnCode::TRY_AGAIN => io::ErrorKind::Interrupted,
			ReturnCode::BAD_ITEM | ReturnCode::USER_UNKNOWN => io::ErrorKind::NotFound,
			ReturnCode::CRED_INSUFFICIENT | ReturnCode::PERM_DENIED => io::ErrorKind::PermissionDenied,
			_ => io::ErrorKind::Other
		}, Box::new(error))
	}
}
