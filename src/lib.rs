//! Pure Rust async implementation of the [Source A2S_INFO Query Protocol](https://developer.valvesoftware.com/wiki/Server_queries#A2S_INFO)
pub mod error;
pub mod info;
pub mod packet;
mod parse;