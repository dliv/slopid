set shell := ["sh", "-euc"]

check:
    just format
    just clippy
    just test
    just adapter-tests
    just policy-control

format:
    cargo fmt --all -- --check

clippy:
    cargo clippy --workspace --locked --all-targets -- -D warnings

test:
    cargo test --workspace --locked

adapter-tests:
    PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts/tests -p 'test_*.py'

policy-control:
    python3 scripts/check_clippy_policy.py
