#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(missing_docs)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::items_after_statements)]
#![allow(
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::missing_const_for_fn,
    clippy::inefficient_to_string,
    clippy::multiple_crate_versions,
    clippy::redundant_pub_crate,
    clippy::use_self
)]

#[macro_use]
extern crate enum_kinds;
#[macro_use]
extern crate log;

pub mod device;

#[cfg(test)]
mod tests {}
