//! Pure Rust async implementation of the [Source A2S_INFO Query Protocol](https://developer.valvesoftware.com/wiki/Server_queries#A2S_INFO)
pub mod error;
pub mod info;
pub mod packet;
mod parse;

use tokio::main;

use crate::info::query;

#[tokio::main]
async fn main() {
    let host = "57.129.13.51:27015";
    let res = query(host).await.unwrap();
    dbg!(res);
}