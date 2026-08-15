# Contributing

Thank you for helping keep egui-wgpu-compat useful and narrowly focused.

Please base renderer changes on the official egui-wgpu implementation whenever
possible, cite the upstream tag or commit in the pull request, and avoid adding
device, surface, window, or event-loop ownership.

Before opening a pull request, run:

```console
cargo fmt --all -- --check
cargo check --all-targets --all-features
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release --all-targets --all-features
```

Use short, imperative commit subjects (for example, “Document floating-point
output”) and keep unrelated changes in separate commits.

