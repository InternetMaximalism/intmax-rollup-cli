# Install Guide (Ubuntu)

## Install some packages

```sh
sudo apt update
sudo apt install git curl build-essential libssl-dev pkg-config
```

## Clone this repository

https://github.com/InternetMaximalism/intmax-rollup-cli

```sh
git clone https://github.com/InternetMaximalism/intmax-rollup-cli.git
cd intmax-rollup-cli
```

## Clone submodules

```sh
git submodule update --init --recursive
```

## Install Rust

https://www.rust-lang.org/tools/install

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh # Enter
source "$HOME/.cargo/env"
cargo --version # cargo 1.65.0-nightly (646e9a0b9 2022-09-02)
```

## Build this CLI

```sh
cp -n example.env .env
cargo build --release
alias intmax='./target/release/intmax'
intmax config aggregator-url https://alpha.testnet.intmax.io/
```
