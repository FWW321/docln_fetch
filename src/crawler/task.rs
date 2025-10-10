use anyhow::Result;
use tokio::task::JoinSet;

// 使用一个结构体而不是单个joinset是为了方便有不同返回值
// 且后续可以方便的将结构体替换为单个joinset，因为全是关联方法
// 而且可以等待不同类型的任务
pub struct TaskManager<R: Send + 'static> {
    tasks: JoinSet<Result<R>>,
}

impl<R: Send + 'static> TaskManager<R> {
    pub fn new() -> Self {
        Self {
            tasks: JoinSet::new(),
        }
    }

    pub fn spawn<F>(&mut self, future: F)
    where
        F: std::future::Future<Output = Result<R>> + Send + 'static,
    {
        self.tasks.spawn(future);
    }

    pub async fn wait(&mut self) -> Result<Vec<R>> {
        let mut results = Vec::new();
        while let Some(res) = self.tasks.join_next().await {
            results.push(res??);
        }
        Ok(results)
    }
}
