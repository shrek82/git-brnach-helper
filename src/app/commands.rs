//! 命令抽象模块
//!
//! Command 封装异步操作，完成时通过 channel 发送消息

use std::sync::mpsc;

/// 命令：封装异步操作
/// 完成时通过 channel 发送消息
pub struct Command<Msg> {
    inner: Option<Box<dyn FnOnce(mpsc::Sender<Msg>) + Send>>,
}

impl<Msg: Send + 'static> Command<Msg> {
    /// 创建执行任务的命令（支持闭包）
    pub fn perform<T, F, M>(task: F, mapper: M) -> Self
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
        M: FnOnce(T) -> Msg + Send + 'static,
    {
        Command {
            inner: Some(Box::new(move |tx| {
                let result = task();
                let _ = tx.send(mapper(result));
            })),
        }
    }

    /// 创建执行会返回 Result 的任务的命令（支持闭包）
    pub fn perform_result<T, E, F, M>(task: F, mapper: M) -> Self
    where
        T: Send + 'static,
        E: Send + 'static,
        F: FnOnce() -> Result<T, E> + Send + 'static,
        M: FnOnce(Result<T, E>) -> Msg + Send + 'static,
    {
        Command {
            inner: Some(Box::new(move |tx| {
                let result = task();
                let _ = tx.send(mapper(result));
            })),
        }
    }

    /// 批量执行多个命令
    pub fn batch(commands: Vec<Self>) -> Self {
        Command {
            inner: Some(Box::new(move |tx| {
                for cmd in commands {
                    if let Some(inner) = cmd.inner {
                        inner(tx.clone());
                    }
                }
            })),
        }
    }

    /// 无操作
    pub fn none() -> Self {
        Command { inner: None }
    }

    /// 执行命令（在后台线程中）
    pub fn execute(self, tx: mpsc::Sender<Msg>) {
        if let Some(inner) = self.inner {
            std::thread::spawn(move || {
                inner(tx);
            });
        }
    }

    /// 立即执行（在当前线程中，用于同步操作）
    pub fn execute_sync(self, tx: mpsc::Sender<Msg>) {
        if let Some(inner) = self.inner {
            inner(tx);
        }
    }
}

impl<Msg: Send + 'static> Default for Command<Msg> {
    fn default() -> Self {
        Self::none()
    }
}
