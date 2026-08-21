/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::ffi::{CStr, CString, c_char, c_void};
use std::future::Future;
use std::pin::Pin;

use http::HeaderValue;
use http::header::CONTENT_TYPE;

use servo_api::protocol_handler::{
    DoneChannel, FetchContext, HttpStatus, NetworkError, ProtocolHandler, Request, Response,
    ResponseBody, ResourceFetchTiming,
};

/// The resource a [`ServoProtocolHandlerCallback`] answers a request with.
#[repr(C)]
pub struct ServoProtocolResponse {
    /// The bytes of the resource. Copied by Servo before the callback returns.
    pub data: *const u8,
    /// The number of bytes at `data`.
    pub len: usize,
    /// The MIME type of the resource, as a NUL terminated UTF-8 string. `text/plain` if `NULL`.
    pub content_type: *const c_char,
}

/// Answers a request for a resource of a registered scheme. `url` is the requested URL as a NUL
/// terminated UTF-8 string. Returns `true` after filling in `response`, or `false` to fail the
/// request.
///
/// # Safety
/// The callback may be invoked from any thread and must not unwind.
pub type ServoProtocolHandlerCallback = unsafe extern "C" fn(
    url: *const c_char,
    response: *mut ServoProtocolResponse,
    user_data: *mut c_void,
) -> bool;

pub(crate) struct CallbackProtocolHandler {
    callback: ServoProtocolHandlerCallback,
    user_data: *mut c_void,
}

// SAFETY: The embedder guarantees, as part of the contract of
// `servo_builder_add_protocol_handler`, that the callback and its user data can be used from any
// thread.
unsafe impl Send for CallbackProtocolHandler {}
unsafe impl Sync for CallbackProtocolHandler {}

impl CallbackProtocolHandler {
    pub(crate) fn new(
        callback: ServoProtocolHandlerCallback,
        user_data: *mut c_void,
    ) -> CallbackProtocolHandler {
        CallbackProtocolHandler {
            callback,
            user_data,
        }
    }
}

impl ProtocolHandler for CallbackProtocolHandler {
    fn load(
        &self,
        request: &mut Request,
        _done_chan: &mut DoneChannel,
        _context: &FetchContext,
    ) -> Pin<Box<dyn Future<Output = Response> + Send>> {
        let url = request.current_url();
        let timing = ResourceFetchTiming::new(request.timing_type());

        let Ok(url_string) = CString::new(url.as_str()) else {
            return Box::pin(std::future::ready(Response::network_error(
                NetworkError::ResourceLoadError("URL is not a valid C string".into()),
            )));
        };

        let mut callback_response = ServoProtocolResponse {
            data: std::ptr::null(),
            len: 0,
            content_type: std::ptr::null(),
        };

        // SAFETY: The embedder upholds the contract documented on
        // `servo_builder_add_protocol_handler`.
        let handled = unsafe {
            (self.callback)(url_string.as_ptr(), &mut callback_response, self.user_data)
        };

        if !handled || callback_response.data.is_null() {
            return Box::pin(std::future::ready(Response::network_error(
                NetworkError::ResourceLoadError("No such resource".into()),
            )));
        }

        // SAFETY: The embedder guarantees `data` points at `len` readable bytes for the duration of
        // the call. The bytes are copied here so that nothing outlives the callback.
        let bytes =
            unsafe { std::slice::from_raw_parts(callback_response.data, callback_response.len) }
                .to_vec();

        let content_type = if callback_response.content_type.is_null() {
            "text/plain".to_owned()
        } else {
            // SAFETY: As above, for the content type string.
            unsafe { CStr::from_ptr(callback_response.content_type) }
                .to_string_lossy()
                .into_owned()
        };

        let mut response = Response::new(url, timing);
        *response.body.lock() = ResponseBody::Done(bytes);
        if let Ok(value) = HeaderValue::from_str(&content_type) {
            response.headers.insert(CONTENT_TYPE, value);
        }
        response.status = HttpStatus::default();

        Box::pin(std::future::ready(response))
    }

    fn is_fetchable(&self) -> bool {
        true
    }
}
