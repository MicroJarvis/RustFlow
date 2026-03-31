use crate::flow::TaskId;

pub trait Observer: Send + Sync {
    fn set_up(&self, _num_workers: usize) {}

    fn task_started(&self, _worker_id: usize, _task_id: TaskId, _task_name: Option<&str>) {}

    fn task_finished(&self, _worker_id: usize, _task_id: TaskId, _task_name: Option<&str>) {}

    fn worker_sleep(&self, _worker_id: usize) {}

    fn worker_wake(&self, _worker_id: usize) {}
}
