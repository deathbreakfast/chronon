#!/bin/bash
# Bench host bootstrap — runs as root via cloud-init; installs Rust for ec2-user.
set -euo pipefail
dnf install -y git gcc openssl-devel
sudo -u ec2-user bash -lc 'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y'
sudo -u ec2-user bash -lc 'source ~/.cargo/env && rustup default stable'
cat >> /home/ec2-user/.bashrc <<'ENV'
export CARGO_BUILD_JOBS=4
export RUST_BACKTRACE=1
ENV
