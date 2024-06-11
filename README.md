# rsourcequery

Pure Rust async implementation of the [Source A2S_INFO Query Protocol](https://developer.valvesoftware.com/wiki/Server_queries#A2S_INFO).

Sample snippet:
```rust
use rsourcequery::info::{ServerInfo, query, query_timeout_duration};
use std::time::Duration;
use tokio::main;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Uncletopia New York City 1
    let host: &str = "nyc-1.us.uncletopia.com:27015";

    // query with a default timeout of 5 seconds
    let info: ServerInfo = query(host).await?;
    dbg!(info);

    // query with a custom duration
    let long_duration: Duration = Duration::from_secs(100000);
    let long_awaited_info: ServerInfo = query_timeout_duration(host, long_duration).await?;
    dbg!(long_awaited_info);
    
    Ok(())
}
```

## To-Do

- [x] Querying
- [x] Single packet response parsing
- [x] Challenge resolution
- [x] Server info parsing
- [x] High-level async `query()`
- [x] String handling improvement
- [ ] Split packet response parsing