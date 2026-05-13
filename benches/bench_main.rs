// Benchmark stubs for Criterion-based performance tests
// Stage 7: Define benchmark targets for key operations

// To run: cargo bench
// Requires Cargo.toml: [dev-dependencies] criterion = "0.5"

// Benchmark targets (documentation):

// 1. Vault Operations
//    - bench_vault_open_10000_files: Open a vault with 10K .md files
//    - bench_vault_list: List 10K notes
//    - bench_vault_read_write: Read and write a note
//    Target: open < 500ms, list < 100ms, read < 1ms

// 2. AST Parsing
//    - bench_parse_1kb: Parse 1KB Markdown
//    - bench_parse_100kb: Parse 100KB Markdown  
//    - bench_parse_with_links: Parse Markdown with 100 [[wikilinks]]
//    Target: 1KB < 0.1ms, 100KB < 5ms

// 3. Vector Store
//    - bench_vectordb_insert_1000: Insert 1000 chunks
//    - bench_vectordb_search_100k: Search among 100K chunks (top-10)
//    Target: search < 100ms, insert < 10ms/chunk

// 4. IPC Round-Trip
//    - bench_ipc_ping: invoke ping and receive pong
//    - bench_ipc_read_note: invoke read_note for 1KB file
//    Target: round-trip < 2ms

// 5. Diff Engine
//    - bench_diff_identical: Diff two identical 10KB documents
//    - bench_diff_modified: Diff with 10% modification
//    Target: < 10ms for 10KB docs

// 6. RAG Pipeline
//    - bench_rag_retrieve: Hybrid retrieval with 100K chunks
//    Target: < 200ms

use std::time::Instant;

/// Simplified benchmark harness (replaces criterion during Stage 7 dev)
pub struct BenchResult {
    pub name: String,
    pub iterations: u64,
    pub total_ms: f64,
    pub avg_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
}

pub fn run_bench<F>(name: &str, iterations: u64, mut f: F) -> BenchResult
where
    F: FnMut() -> (),
{
    let mut times = Vec::with_capacity(iterations as usize);

    // Warmup (10%)
    for _ in 0..(iterations / 10).max(1) {
        f();
    }

    // Measurement
    for _ in 0..iterations {
        let start = Instant::now();
        f();
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        times.push(elapsed);
    }

    let total_ms: f64 = times.iter().sum();
    let avg_ms = total_ms / iterations as f64;
    let min_ms = times.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_ms = times.iter().cloned().fold(0.0_f64, f64::max);

    BenchResult {
        name: name.to_string(),
        iterations,
        total_ms,
        avg_ms,
        min_ms,
        max_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bench_basic() {
        let result = run_bench("test_add", 100, || {
            let _sum: u64 = (0..1000).sum();
        });

        assert_eq!(result.iterations, 100);
        assert!(result.avg_ms > 0.0);
        assert!(result.min_ms <= result.avg_ms);
        assert!(result.max_ms >= result.avg_ms);
    }

    #[test]
    fn test_bench_vault_simulation() {
        // Simulate vault read performance
        let content = "# Test\n".repeat(1000); // ~7KB
        let result = run_bench("vault_read_sim", 200, || {
            let _lines: Vec<&str> = content.lines().collect();
        });

        assert!(result.avg_ms < 10.0, "Vault read should be fast");
    }

    #[test]
    fn test_bench_parser_simulation() {
        // Simulate AST parser performance
        let md = "# Title\n\nParagraph with [[link]].\n".repeat(100);
        let result = run_bench("parser_sim", 200, || {
            let _link_count = md.matches("[[").count();
        });

        assert!(_ = result.avg_ms < 5.0, "Parser should be fast");
    }
}
