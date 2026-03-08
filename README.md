# Working Example for Axs15231b on JC3248W535EN

This repository demonstrates a working example with embedded-graphics for the Axs15231b display on the JC3248W535EN using bare-metal Rust.

## Notes

- Bare-metal ESP32-S3: This project uses the esp-hal crate for bare-metal development on the ESP32-S3 specify.

- QSPI (24.6ms per full frame): To improve transfer speed, QSPI is used instead of SPI. Since the embedded-hal crate does not provide a trait for QSPI, the ESP-HAL specific types are used.

- LVGL rust binding: I attempted to use the Rust binding for LVGL, but it was difficult to work with. If you want to try it, you can copy the contents of the old_lvgl_example folder into src/.

- Mac M4 / ESP32-S3 compilation: To compile LVGL on macOS M4 with ESP32-S3, I added the BINDGEN_EXTRA_CLANG_ARGS environment variable in .cargo/config.toml. Make sure to update the path according to your system setup.
