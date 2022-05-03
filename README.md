# Rust SDL

Playing around with Rust and SDL2 and maybe OpenCV.

## Usage

Press `c` to start creating a region, `c` again to finish, `x` to cancel.

## Notes

* Install `opencv4` before doing `cargo build` (`brew install opencv@4`).
* `opencv` seems to have issues on Mac OS, I had to [follow the advice
on finding `libclang.dylib`](https://lib.rs/crates/opencv) and run
this command: ` export DYLD_FALLBACK_LIBRARY_PATH="$(xcode-select
--print-path)/Toolchains/XcodeDefault.xctoolchain/usr/lib/"`