//! UI 线程「正在执行可能持续数秒的操作」的共享标记。
//!
//! GPUI 的周期任务（快捷键 60ms、托盘 120ms、播放进度 250ms）会在 UI 线程持有
//! 实体可变更借用时被调度（例如阻塞对话框嵌套消息循环、音频重定位期间）。
//! 此时周期任务去读写同一个实体会 panic（`already mutably borrowed`），
//! 而 panic 一旦穿过 `extern "system"` 的窗口过程就会 `panic in a function that
//! cannot unwind`，进程**直接消失且没有提示**。
//!
//! 因此长阻塞操作进入前沿一个 [`enter`]（返回的 guard 在作用域结束时自动清除），
//! 周期任务看到 [`is_busy`] 就跳过这一次轮询，等下一拍再试。

use std::sync::atomic::{AtomicBool, Ordering};

static BUSY: AtomicBool = AtomicBool::new(false);

/// 进入长阻塞操作，guard 存活期间 [`is_busy`] 为真。
pub struct BusyGuard {
    _private: (),
}

/// 标记 UI 线程开始一个可能阻塞数秒的操作。
pub fn enter() -> BusyGuard {
    BUSY.store(true, Ordering::SeqCst);
    BusyGuard { _private: () }
}

/// 当前是否有长阻塞操作正在进行。
pub fn is_busy() -> bool {
    BUSY.load(Ordering::SeqCst)
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::SeqCst);
    }
}
