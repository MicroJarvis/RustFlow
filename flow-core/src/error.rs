use std::fmt;

use crate::flow::TaskId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowError {
    task_id: Option<TaskId>,
    task_name: Option<String>,
    message: String,
}

impl FlowError {
    pub(crate) fn panic(task_id: TaskId, task_name: Option<&str>, message: String) -> Self {
        Self {
            task_id: Some(task_id),
            task_name: task_name.map(ToOwned::to_owned),
            message,
        }
    }

    pub(crate) fn plain(message: impl Into<String>) -> Self {
        Self {
            task_id: None,
            task_name: None,
            message: message.into(),
        }
    }

    pub fn task_id(&self) -> Option<TaskId> {
        self.task_id
    }

    pub fn task_name(&self) -> Option<&str> {
        self.task_name.as_deref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.task_id, self.task_name.as_deref()) {
            (Some(task_id), Some(task_name)) => {
                write!(f, "task {task_id} ({task_name}) failed: {}", self.message)
            }
            (Some(task_id), None) => write!(f, "task {task_id} failed: {}", self.message),
            (None, _) => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for FlowError {}
