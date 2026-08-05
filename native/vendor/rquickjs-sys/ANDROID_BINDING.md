# Android binding

`rquickjs-sys` 0.12.2 does not publish a binding named
`aarch64-linux-android.rs`. Android arm64 uses the same LP64 C ABI as the
published `aarch64-unknown-linux-gnu.rs` binding, so that generated binding is
also provided under the Android target name in this vendored patch.

The remaining source is unchanged from the crates.io `rquickjs-sys` 0.12.2
package. Remove this patch after upstream publishes an Android binding.
