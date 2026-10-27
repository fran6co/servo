/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

unsafe extern "C" {
    fn run_c_api_tests() -> i32;
    fn run_c_integration_tests() -> i32;
    fn run_c_up_to_date_frame_tests() -> i32;
}

pub fn c_api_tests() {
    let result = unsafe { run_c_api_tests() };
    assert_eq!(result, 0, "C API tests failed");
}

pub fn c_integration_tests() {
    let result = unsafe { run_c_integration_tests() };
    assert_eq!(result, 0, "C integration tests failed");
}

pub fn c_up_to_date_frame_tests() {
    let result = unsafe { run_c_up_to_date_frame_tests() };
    assert_eq!(result, 0, "C up-to-date frame tests failed");
}
