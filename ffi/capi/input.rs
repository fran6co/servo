/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::ffi::{CStr, CString, c_char, c_void};

use servo_api::{
    DevicePoint, InputEvent, JSValue, Key, KeyState, KeyboardEvent, MouseButton,
    MouseButtonAction, MouseButtonEvent, MouseMoveEvent, NamedKey, WebView, WheelDelta, WheelEvent,
    WheelMode,
};

#[repr(C)]
pub enum ServoMouseButton {
    Left = 0,
    Middle = 1,
    Right = 2,
}

impl From<ServoMouseButton> for MouseButton {
    fn from(button: ServoMouseButton) -> Self {
        match button {
            ServoMouseButton::Left => MouseButton::Left,
            ServoMouseButton::Middle => MouseButton::Middle,
            ServoMouseButton::Right => MouseButton::Right,
        }
    }
}

#[repr(C)]
pub enum ServoNamedKey {
    Enter = 0,
    Tab = 1,
    Backspace = 2,
    Delete = 3,
    Escape = 4,
    ArrowLeft = 5,
    ArrowRight = 6,
    ArrowUp = 7,
    ArrowDown = 8,
    Home = 9,
    End = 10,
    PageUp = 11,
    PageDown = 12,
}

impl From<ServoNamedKey> for NamedKey {
    fn from(key: ServoNamedKey) -> Self {
        match key {
            ServoNamedKey::Enter => NamedKey::Enter,
            ServoNamedKey::Tab => NamedKey::Tab,
            ServoNamedKey::Backspace => NamedKey::Backspace,
            ServoNamedKey::Delete => NamedKey::Delete,
            ServoNamedKey::Escape => NamedKey::Escape,
            ServoNamedKey::ArrowLeft => NamedKey::ArrowLeft,
            ServoNamedKey::ArrowRight => NamedKey::ArrowRight,
            ServoNamedKey::ArrowUp => NamedKey::ArrowUp,
            ServoNamedKey::ArrowDown => NamedKey::ArrowDown,
            ServoNamedKey::Home => NamedKey::Home,
            ServoNamedKey::End => NamedKey::End,
            ServoNamedKey::PageUp => NamedKey::PageUp,
            ServoNamedKey::PageDown => NamedKey::PageDown,
        }
    }
}

/// # Safety
/// `webview` must be a live handle from `servo_webview_builder_build`, used on its creating thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn servo_webview_notify_mouse_move(webview: *mut WebView, x: f32, y: f32) {
    assert!(!webview.is_null(), "webview pointer must not be null");
    let webview = unsafe { &*webview };

    webview.notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(
        DevicePoint::new(x, y).into(),
    )));
}

/// # Safety
/// See [`servo_webview_notify_mouse_move`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn servo_webview_notify_mouse_button(
    webview: *mut WebView,
    x: f32,
    y: f32,
    button: ServoMouseButton,
    down: bool,
) {
    assert!(!webview.is_null(), "webview pointer must not be null");
    let webview = unsafe { &*webview };

    let action = if down {
        MouseButtonAction::Down
    } else {
        MouseButtonAction::Up
    };

    webview.notify_input_event(InputEvent::MouseButton(MouseButtonEvent::new(
        action,
        button.into(),
        DevicePoint::new(x, y).into(),
    )));
}

/// # Safety
/// See [`servo_webview_notify_mouse_move`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn servo_webview_notify_wheel(
    webview: *mut WebView,
    x: f32,
    y: f32,
    delta_x: f64,
    delta_y: f64,
) {
    assert!(!webview.is_null(), "webview pointer must not be null");
    let webview = unsafe { &*webview };

    let delta = WheelDelta {
        x: delta_x,
        y: delta_y,
        z: 0.0,
        mode: WheelMode::DeltaLine,
    };

    webview.notify_input_event(InputEvent::Wheel(WheelEvent::new(
        delta,
        DevicePoint::new(x, y).into(),
    )));
}

/// # Safety
/// See [`servo_webview_notify_mouse_move`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn servo_webview_notify_named_key(
    webview: *mut WebView,
    key: ServoNamedKey,
    down: bool,
) {
    assert!(!webview.is_null(), "webview pointer must not be null");
    let webview = unsafe { &*webview };

    let state = if down { KeyState::Down } else { KeyState::Up };

    webview.notify_input_event(InputEvent::Keyboard(KeyboardEvent::from_state_and_key(
        state,
        Key::Named(key.into()),
    )));
}

/// # Safety
/// See [`servo_webview_notify_mouse_move`]. `text` must be a NUL terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn servo_webview_notify_text(webview: *mut WebView, text: *const c_char) {
    assert!(!webview.is_null(), "webview pointer must not be null");
    assert!(!text.is_null(), "text pointer must not be null");
    let webview = unsafe { &*webview };

    let Ok(text) = unsafe { CStr::from_ptr(text) }.to_str() else {
        log::error!("servo_webview_notify_text: text is not valid UTF-8");
        return;
    };

    for character in text.chars() {
        let key = Key::Character(character.to_string());
        for state in [KeyState::Down, KeyState::Up] {
            webview.notify_input_event(InputEvent::Keyboard(KeyboardEvent::from_state_and_key(
                state,
                key.clone(),
            )));
        }
    }
}

/// # Safety
/// See [`servo_webview_notify_mouse_move`]. `script` must be a NUL terminated UTF-8 string.
/// `callback` must not unwind, and `user_data` must stay valid until it is invoked. `result` is
/// `NULL` if evaluation failed and is only valid for the duration of the callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn servo_webview_evaluate_javascript(
    webview: *mut WebView,
    script: *const c_char,
    callback: Option<unsafe extern "C" fn(result: *const c_char, user_data: *mut c_void)>,
    user_data: *mut c_void,
) {
    assert!(!webview.is_null(), "webview pointer must not be null");
    assert!(!script.is_null(), "script pointer must not be null");
    let webview = unsafe { &*webview };

    let Ok(script) = unsafe { CStr::from_ptr(script) }.to_str() else {
        log::error!("servo_webview_evaluate_javascript: script is not valid UTF-8");
        return;
    };

    let user_data = user_data as usize;
    webview.evaluate_javascript(script, move |result| {
        let Some(callback) = callback else {
            return;
        };

        let text = match result {
            Ok(value) => CString::new(js_value_to_string(&value)).ok(),
            Err(error) => {
                log::warn!("JavaScript evaluation failed: {error:?}");
                None
            },
        };
        let result = text.as_ref().map_or(std::ptr::null(), |text| text.as_ptr());

        unsafe {
            callback(result, user_data as *mut c_void);
        }
    });
}

fn js_value_to_string(value: &JSValue) -> String {
    match value {
        JSValue::Undefined => "undefined".into(),
        JSValue::Null => "null".into(),
        JSValue::Boolean(value) => value.to_string(),
        JSValue::Number(value) => value.to_string(),
        JSValue::String(value) => value.clone(),
        JSValue::Element(value) |
        JSValue::ShadowRoot(value) |
        JSValue::Frame(value) |
        JSValue::Window(value) => value.clone(),
        JSValue::Array(values) => {
            let values: Vec<String> = values.iter().map(js_value_to_string).collect();
            format!("[{}]", values.join(","))
        },
        JSValue::Object(entries) => {
            let entries: Vec<String> = entries
                .iter()
                .map(|(key, value)| format!("{key}:{}", js_value_to_string(value)))
                .collect();
            format!("{{{}}}", entries.join(","))
        },
    }
}
