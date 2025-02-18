// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

#[cfg_attr(target_os = "linux", path = "linux/transport.rs")]
#[cfg_attr(target_os = "macos", path = "linux/transport_poll.rs")]
#[cfg_attr(target_os = "windows", path = "win/transport.rs")]
pub mod transport_poll;
