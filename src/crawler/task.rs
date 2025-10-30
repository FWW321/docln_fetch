use anyhow::Result;
use tokio::task::JoinSet;

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
