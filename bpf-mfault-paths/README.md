This is an example of libbpf-based profiler.

It tries to catch sequential reads on mmaps that incorrectly marked as `MADV_RANDOM`.

Usage:

1. `cargo build --release`.
2. Run Qdrant with some large collection.
3. `sudo ./target/release/mmfault_paths --pid "$(pidof qdrant)" --min-faults 200`.
4. Trigger re-indexing in Qdrant.
5. The profiler will print stacktraces and file names that triggered at least 200 sequential page faults.

---

For LSP/editor support for `*.bpf.c`, compile the whole project using [bear](https://github.com/rizsotto/Bear) and use `clangd` as LSP server:

```bash
cargo clean; bear --append -- cargo build --release
```

For proper stacktraces, compile Qdrant with frame pointers and line debug info:

```toml
# .cargo/config.toml
[build]
rustflags = ["-C", "force-frame-pointers=yes"]
```

```toml
# Cargo.toml
[profile.perf]
debug = "line-tables-only"
```
