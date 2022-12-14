use thiserror::Error;

pub mod server;
pub mod client;

#[derive(Error, Debug)]
pub enum Error {}