// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! External xtasks. (extasks?)

use std::ffi::{OsStr, OsString};
use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::{Context, Result};
use clap::Parser;
use dev_tools_common::{CargoLocation, cargo_command};

/// Argument parser for external xtasks.
///
/// In general we want all developer tasks to be discoverable simply by running
/// `cargo xtask`, but some development tools end up with a particularly
/// large dependency tree. It's not ideal to have to pay the cost of building
/// our release engineering tooling if all the user wants to do is check for
/// workspace dependency issues.
///
/// `External` provides a pattern for creating xtasks that live in other crates.
/// An external xtask is defined on `crate::Cmds` as a tuple variant containing
/// `External`, which captures all arguments and options (even `--help`) as
/// a `Vec<OsString>`. The main function then calls `External::exec` with the
/// appropriate bin target name and any additional Cargo arguments.
#[derive(Parser)]
#[clap(
    disable_help_flag(true),
    disable_help_subcommand(true),
    disable_version_flag(true)
)]
pub struct External {
    #[clap(trailing_var_arg(true), allow_hyphen_values(true))]
    args: Vec<OsString>,

    // This stores an in-progress Command builder. `cargo_args` appends args
    // to it, and `exec` consumes it. Clap does not treat this as a command
    // (`skip`), but fills in this field by calling `new_command`.
    #[clap(skip = new_command())]
    command: Command,
}

impl External {
    /// Add additional arguments to `cargo run` (for instance, to run the
    /// external xtask in release mode).
    pub fn cargo_args(
        mut self,
        args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    ) -> External {
        self.command.args(args);
        self
    }

    pub fn trailing_args(&self) -> &[OsString] {
        &self.args
    }

    pub fn exec_example(self, example_target: impl AsRef<OsStr>) -> Result<()> {
        self.exec_common("--example", example_target.as_ref())
    }

    pub fn exec_bin(self, bin_target: impl AsRef<OsStr>) -> Result<()> {
        self.exec_common("--bin", bin_target.as_ref())
    }

    fn exec_common(mut self, kind: &'static str, target: &OsStr) -> Result<()> {
        let error =
            self.command.arg(kind).arg(target).arg("--").args(self.args).exec();
        Err(error).context("failed to exec `cargo run`")
    }
}

fn new_command() -> Command {
    let mut command = cargo_command(CargoLocation::FromEnv);
    command.arg("run");
    command
}
