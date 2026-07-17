#!/bin/bash
set -euo pipefail
dnf install -y git docker gcc openssl-devel
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source /home/ec2-user/.cargo/env
rustup default stable

cat >> /home/ec2-user/.bashrc <<'ENV'
export CARGO_BUILD_JOBS=1
export RUST_BACKTRACE=1
ENV
