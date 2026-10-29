use std::time::Instant;
use solana_trading_terminal::db::Database;

#[tokio::main]
async fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let t = Instant::now();
    println!("open start {:?}", dir);
    match Database::open(&dir).await {
        Ok(db) => {
            println!("open ok in {:?}", t.elapsed());
            println!("verify: {:?}", db.verify().await.map(|_| "ok"));
        }
        Err(e) => println!("open failed: {e}"),
    }
}
