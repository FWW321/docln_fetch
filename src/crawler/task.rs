use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;

// OnceLock解决在多线程安全初始化的问题
// Mutex解决多线程访问时的互斥问题
static TASK_MANAGER: OnceLock<Mutex<CrawlerTaskManager>> = OnceLock::new();

// 使用一个结构体而不是单个joinset是为了方便有不同返回值
// 且后续可以方便的将结构体替换为单个joinset，因为全是关联方法
// 而且可以等待不同类型的任务
pub struct CrawlerTaskManager {
    // 暂时不需要返回值
    chapter_tasks: JoinSet<()>,  // 章节下载任务
    image_tasks: JoinSet<()>,    // 图片下载任务
    max_concurrent_tasks: usize, // 最大并发任务数
    semaphore: Arc<Semaphore>,   // 控制并发任务数
}

impl CrawlerTaskManager {
    pub fn new(max_concurrent_tasks: usize) -> Self {
        Self {
            chapter_tasks: JoinSet::new(),
            image_tasks: JoinSet::new(),
            max_concurrent_tasks,
            semaphore: Arc::new(Semaphore::new(max_concurrent_tasks)),
        }
    }

    pub fn get_instance() -> &'static Mutex<CrawlerTaskManager> {
        TASK_MANAGER.get_or_init(|| Mutex::new(CrawlerTaskManager::new(5)))
    }

    pub fn spawn_chapter<F>(future: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        // Self::get_instance().lock().unwrap().chapter_tasks.spawn(future);
        // 如果不传入闭包，则需要在每次调用时都获取锁
        // 而函数里需要信号量，也需要获取锁
        // 直接传入闭包，在函数内部统一获取锁
        Self::spawn_task_internal(future, |manager| &mut manager.chapter_tasks);
    }

    pub fn spawn_image<F>(future: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        // Self::get_instance().lock().unwrap().image_tasks.spawn(future);
        Self::spawn_task_internal(future, |manager| &mut manager.image_tasks);
    }

    fn spawn_task_internal<F, G>(future: F, task_set_selector: G)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
        G: FnOnce(&mut CrawlerTaskManager) -> &mut JoinSet<()>,
    {
        let manager = Self::get_instance();
        let mut manager_lock = manager.lock().unwrap();

        let semaphore = manager_lock.semaphore.clone();

        // 包装原始future，添加并发控制
        let controlled_future = async move {
            // 获取信号量许可，如果达到最大并发数会等待
            let _permit = semaphore.acquire().await;
            future.await;
            // _permit 在这里被drop，自动释放许可
        };

        // 将受控的任务添加到对应的任务集
        let task_set = task_set_selector(&mut manager_lock);
        task_set.spawn(controlled_future);
    }

    pub fn set_max_concurrent_tasks(&mut self, max_tasks: usize) {
        self.max_concurrent_tasks = max_tasks;
        // 创建新的信号量来替换旧的
        // 可以在运行时替换，Arc保证旧的信号量在没有任务使用时才会被释放
        self.semaphore = Arc::new(Semaphore::new(max_tasks));
    }

    // 获取当前最大并发数
    pub fn get_max_concurrent_tasks(&self) -> usize {
        self.max_concurrent_tasks
    }

    // 获取当前可用并发数
    pub fn available_concurrent_tasks(&self) -> usize {
        self.semaphore.available_permits()
    }

    async fn wait_tasks(mut tasks: JoinSet<()>) {
        while let Some(res) = tasks.join_next().await {
            if let Err(e) = res {
                eprintln!("任务出错: {}", e);
            }
        }
    }

    // 避免在持有锁的情况下等待异步任务
    // 如果在其他异步任务里面需要获取锁，则会导致死锁
    // 错误：Self::get_instance().lock().unwrap().wait_all().await;
    pub async fn wait_all_tasks() {
        // 所有章节任务都是在同步代码中添加的
        // 只要保证等待任务在所有章节任务添加之后即可
        let chapter_tasks = {
            let mut manager = Self::get_instance().lock().unwrap();
            std::mem::take(&mut manager.chapter_tasks)
        };

        Self::wait_tasks(chapter_tasks).await;
        // 图片任务在章节任务中添加
        // 先要保证章节任务全部完成，再等待图片任务
        // 否则可能图片任务还没添加完就开始等待，导致等待不完整
        let image_tasks = {
            let mut manager = Self::get_instance().lock().unwrap();
            std::mem::take(&mut manager.image_tasks)
        };
        Self::wait_tasks(image_tasks).await;
    }
}
