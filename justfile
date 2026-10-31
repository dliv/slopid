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
    cargo clippy --locked --all-targets -- -D warnings

test:
    cargo test --locked

adapter-tests:
    PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts/tests -p 'test_*.py'

policy-control:
    python3 scripts/check_clippy_policy.py
