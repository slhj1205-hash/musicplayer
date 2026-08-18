use lyre_core::Library;
fn main() {
    let root = std::env::args().nth(1).unwrap();
    let cache = std::env::args().nth(2).unwrap();
    let runs: usize = std::env::args().nth(3).map(|s| s.parse().unwrap()).unwrap_or(1);
    for i in 0..runs {
        let t = std::time::Instant::now();
        let (lib, stats) = Library::scan(&root, &cache).unwrap();
        println!("run {i}: {:?} songs={} hits={} reprobed={} skipped={} cache_bytes={}",
            t.elapsed(), lib.len(), stats.cache_hits, stats.reprobed, stats.skipped(),
            std::fs::metadata(&cache).map(|m| m.len()).unwrap_or(0));
    }
}
