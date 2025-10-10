use std::sync::OnceLock;
use std::thread::available_parallelism;

use anyhow::Result;
use tokio::sync::RwLock;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::info;
use tracing::instrument;

// OnceLock解决在多线程安全初始化的问题
// Mutex解决多线程访问时的互斥问题
// static MAX_TASKS: StdRwMutex<usize> = StdRwMutex::new(24);
static SEMAPHORE: OnceLock<RwLock<Semaphore>> = OnceLock::new();

#[instrument]
fn get_semaphore() -> &'static RwLock<Semaphore> {
    SEMAPHORE.get_or_init(|| RwLock::new(Semaphore::new(available_parallelism().map_or(12, |n| {
        info!("设置最大并发数为 {}", n.get());
        n.get()
}))))
}

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
        self.spawn_task_internal(future);
    }

    fn spawn_task_internal<F>(&mut self, future: F)
    where
        F: std::future::Future<Output = Result<R>> + Send + 'static,
    {
        // 包装原始future，添加并发控制
        // 如果volume future内需要等待chapter future
        // 则如果许可被耗尽，而volume future又在等待chapter future
        // 则会导致死锁
        // 如果都在同步代码中等待，则不会死锁
        // 或者每个task manager使用独立的信号量
        // 但这样会导致总并发数不可控
        // 或者不使用信号量，反正tokio本身也会调度任务
        let controlled_future = async move {
            let semaphore = get_semaphore().read().await;
            // 获取信号量许可，如果达到最大并发数会等待
            let _permit = semaphore.acquire().await;
            future.await
            // _permit 在这里被drop，自动释放许可
        };

        // 将受控的任务添加到对应的任务集
        self.tasks.spawn(controlled_future);
    }

    // pub fn add_permits(n: usize) {
    //     {
    //         let mut max_tasks = MAX_TASKS.write().unwrap();
    //         *max_tasks += n;
    //     }
    //     let semaphore = get_semaphore();
    //     let semaphore = semaphore.blocking_write();
    //     semaphore.add_permits(n);
    // }

    // pub async fn reduce_permits(n: usize) -> Result<()> {
    //     if n >= *MAX_TASKS.read().unwrap() {
    //         return Err(anyhow::anyhow!(
    //             "不能减少超过当前最大并发数: {}",
    //             *MAX_TASKS.read().unwrap()
    //         ));
    //     }
    //     {
    //         let mut max_tasks = MAX_TASKS.write().unwrap();
    //         *max_tasks -= n;
    //     }
    //     let semaphore = get_semaphore();
    //     let semaphore = semaphore.write().await;
    //     for _ in 0..n {
    //         let permit = semaphore.acquire().await?;
    //         permit.forget();
    //     }
    //     Ok(())
    // }

    pub async fn wait(&mut self) -> Result<Vec<R>> {
        let mut results = Vec::new();
        while let Some(res) = self.tasks.join_next().await {
            results.push(res??);
        }
        Ok(results)
    }
}
