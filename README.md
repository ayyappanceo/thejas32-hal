
# THEJAS32 Rust HAL & Embassy Async Runtime

[![Rust no_std](https://img.shields.io/badge/Rust-no_std-orange.svg)](https://www.rust-lang.org/)
[![Target](https://img.shields.io/badge/target-riscv32i--unknown--none--elf-blue.svg)](https://doc.rust-lang.org/stable/rustc/platform-support/riscv32i-unknown-none-elf.svg)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-green.svg)](LICENSE)

A high-performance, memory-safe `no_std` Hardware Abstraction Layer (HAL) and asynchronous multitasking runtime built for the **THEJAS32 RISC-V SoC**. This crate integrates the **Embassy** async executor (`arch-spin`) and `embassy-time` timer driver directly with bare-metal RISC-V core-level interrupt controllers (CLINT) and custom simulated peripherals.

---

## 🚀 Key Architectural Features

* **`no_std` Core Compatibility:** Engineered specifically for resource-constrained, single-issue `riscv32i` cores without hardware atomics (leveraging `portable-atomic`).
* **Embassy Async Integration:** Full async-await support for concurrent device drivers, periodic background tasks, and low-latency task scheduling.
* **Deterministic CLINT Timer Driver:** Direct hardware trap routing (`mtvec` manipulation) and real-time polling to manage hardware alarms and eliminate idle-loop CPU starvation.
* **Comprehensive Peripheral Support:**
  * **UART0:** High-speed polled and buffered logging / transmission.
  * **SPI0:** Master SPI bus transactions with configurable prescalers.
  * **I2C0:** Multi-operation transactions supporting complex sensor integrations (e.g., MPU6050, SSD1306 displays).
  * **GPIO0:** 16-bit wide-register digital input/output pin manipulation.

---

## 📂 Repository Structure

``text
thejas32-hal/
├── src/
│   └── lib.rs        # HAL drivers, async time driver, and trap handler
├── Cargo.toml        # Crate dependencies and metadata
└── README.md         # Project documentation

🛠️ Prerequisites & Toolchain
To build and simulate this HAL and its associated firmware demos, ensure you have the following installed:

Rust Toolchain: nightly or stable compiler with the bare-metal target:

Bash
rustup target add riscv32i-unknown-none-elf
Renode Simulator: Version 1.17.0+ with support for native C# Roslyn compilation on the fly.

🧪 Running the Renode Simulation
This project is fully co-simulated in Renode using native C# peripheral models (IDoubleWordPeripheral and IWordPeripheral) that map directly to the THEJAS32 memory map.

Clone the repository and navigate to your simulation workspace.

Launch Renode with the provided script:

Bash
renode aries_v3.resc
Verify the console output:

Plaintext
=== THEJAS32 Embassy Async + SPI/I2C HAL ===
[SPI0] TX=[0xAA, 0x55] -> RX=[0x55, 0xAA]
[I2C0] MPU6050 WHO_AM_I = 0x68
[I2C0] SSD1306 Display ON packet sent
=== All Async Tasks & Drivers Verified ===
