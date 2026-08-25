/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

fn main() {
    // Without a soname, linkers record the path they were handed as the dependency, so an embedder
    // that ships the library cannot move it. Darwin gets an install name from rustc already, and
    // Windows DLLs carry their own name.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if !matches!(target_os.as_str(), "windows" | "macos" | "ios") {
        println!("cargo::rustc-link-arg-cdylib=-Wl,-soname,libservo_capi.so");
    }
}
