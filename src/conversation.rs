//! Conversation trait definition module

/***********************************************************************
 * (c) 2021 Christoph Grenz <christophg+gitorious @ grenz-bonn.de>     *
 *                                                                     *
 * This Source Code Form is subject to the terms of the Mozilla Public *
 * License, v. 2.0. If a copy of the MPL was not distributed with this *
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.            *
 ***********************************************************************/

use crate::error::ErrorCode;
use std::ffi::{CString, CStr};
use std::result::Result;

/// Trait for PAM conversation functions
///
/// Implement this for custom behaviour when a PAM module asks for usernames,
/// passwords, etc. or wants to show a message to the user
#[rustversion::attr(since(1.48), doc(alias = "pam_conv"))]
pub trait ConversationHandler {
	/// Called by [`Context`][`crate::Context`] directly after taking ownership
	/// of the handler.
	///
	/// May be called multiple times if
	/// [`Context::replace_conversation()`][`crate::Context::replace_conversation`]
	/// is used. In this case it is called each time a context takes ownership
	/// and passed the current target username of that context (if any) as the
	/// argument.
	///
	/// The default implementation does nothing.
	fn init(&mut self, _default_user: Option<impl AsRef<str>>) {}

	/// Obtains a string whilst echoing text (e.g. username)
	///
	/// # Errors
	/// You should return one of the following error codes on failure.
	/// - [`ErrorCode::CONV_ERR`]: Conversation failure.
	/// - [`ErrorCode::BUF_ERR`]: Memory allocation error.
	/// - [`ErrorCode::CONV_AGAIN`]: no result yet, the PAM library should
	///   pass [`ErrorCode::INCOMPLETE`] to the application and let it
	///   try again later.
	fn prompt_echo_on(&mut self, prompt: &CStr) -> Result<CString, ErrorCode>;

	/// Obtains a string without echoing any text (e.g. password)
	///
	/// # Errors
	/// You should return one of the following error codes on failure.
	/// - [`ErrorCode::CONV_ERR`]: Conversation failure.
	/// - [`ErrorCode::BUF_ERR`]: Memory allocation error.
	/// - [`ErrorCode::CONV_AGAIN`]: no result yet, the PAM library should
	///   pass [`ErrorCode::INCOMPLETE`] to the application and let it
	///   try again later.
	fn prompt_echo_off(&mut self, prompt: &CStr) -> Result<CString, ErrorCode>;

	/// Displays some text.
	fn text_info(&mut self, msg: &CStr);

	/// Displays an error message.
	fn error_msg(&mut self, msg: &CStr);

	/// Obtains a yes/no answer (Linux specific).
	///
	/// The default implementation calls `prompt_echo_on` and maps any answer
	/// starting with 'y' or 'j' to "yes" and everything else to "no".
	fn radio_prompt(&mut self, prompt: &CStr) -> Result<bool, ErrorCode> {
		let prompt = [ prompt.to_bytes(), b" [y/N]\0" ].concat();

		self.prompt_echo_on(CStr::from_bytes_with_nul(&prompt).unwrap())
			.map(|s| matches!(s.as_bytes_with_nul()[0], b'Y' | b'y' | b'j' | b'J'))
	}
}
