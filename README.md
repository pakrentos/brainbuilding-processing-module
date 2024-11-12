# BrainBuilding Module

A real-time EEG processing module with feedback capabilities, based on [labstreaminglayer/liblsl-rust](https://github.com/labstreaminglayer/liblsl-rust) and [tonyke-bot/burberry](https://github.com/tonyke-bot/burberry) with significant modifications for brain-computer interface applications.

## Build
### System Dependencies
- A C/C++ compiler (gcc/clang)
- cmake

### Prerequisites
- Install Rust using rustup (https://rustup.rs/) (don't forget to add the `~/.cargo/bin` directory to your PATH):
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    ```

- Add the `nightly` toolchain:
    ```bash
    rustup toolchain install nightly
    ```

- Fetch the submodules:
    ```bash
    git submodule update --init
    ```

### Build with cargo:
```bash
cargo +nightly build --release
```

## Run
```bash
target/release/brainbuilding-module
```

or 

```bash
cargo +nightly run --release
```


## Features
- Real-time EEG data processing
- Common Spatial Pattern (CSP) filtering
- Support Vector Machine (SVM) classification
- LSL (Lab Streaming Layer) integration
- TCP-based feedback system

UNDER HEAVY DEVELOPMENT.
