# Android binding

`rquickjs-sys` 0.12.2 does not publish a binding named
`aarch64-linux-android.rs`. Android arm64 uses the same LP64 C ABI as the
published `aarch64-unknown-linux-gnu.rs` binding, so that generated binding is
also provided under the Android target name in this vendored patch.

`x86_64-unknown-linux-gnu.rs` is kept for the same reason on CI: `rquickjs` 打开
了 `macro` feature，`rquickjs-macro` 是 proc-macro，构建 Android 目标时 cargo
还要为**宿主机**（ubuntu runner 的 x86_64-unknown-linux-gnu）编译一份
`rquickjs-sys`，缺这份 binding 会让 `cargo ndk build` 直接失败。

The remaining source is unchanged from the crates.io `rquickjs-sys` 0.12.2
package. Remove this patch after upstream publishes an Android binding.
