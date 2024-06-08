# sourcon

Pure Rust async implementation of the [Source A2S_INFO Query Protocol](https://developer.valvesoftware.com/wiki/Server_queries#A2S_INFO).

```rust
use rsourcequery::query::query;
use tokio::main;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let host: &str = "nyc-1.us.uncletopia.com:27015"; // Uncletopia New York City 1
    let info: ServerInfo = query(host, None).await?;
    dbg!(info);
    Ok(())
}
```

## To-Do

[x] Querying
[x] Single packet response parsing
[x] Challenge resolution
[x] Server info parsing
[x] High-level async `query()`
[ ] String handling improvement
[ ] Split packet response parsing