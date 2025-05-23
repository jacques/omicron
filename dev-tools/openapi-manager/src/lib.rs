// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! OpenAPI manager for Omicron.
//!
//! This tool generates and checks OpenAPI specifications for Omicron OpenAPI
//! documents. In the future, all OpenAPI documents will be generated by this
//! tool, but work to make that happen is ongoing.
//!
//! This is meant to be invoked as `cargo xtask openapi`, but is a separate
//! binary to avoid compiling a bunch of extra code when running `cargo xtask`.

mod apis;
mod cmd;
mod compatibility;
mod environment;
mod git;
mod iter_only;
mod omicron;
mod output;
mod resolved;
mod spec_files_blessed;
mod spec_files_generated;
mod spec_files_generic;
mod spec_files_local;
mod validation;

#[macro_use]
extern crate newtype_derive;

pub use cmd::dispatch::*;
