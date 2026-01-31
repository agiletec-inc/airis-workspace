//! Parallel DAG Build Engine
//!
//! Executes build tasks in parallel respecting dependency order.
//! Uses tokio for async execution with configurable worker pool.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, Semaphore};

/// Task state in the execution graph
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskState {
    /// Waiting for dependencies
    Pending,
    /// Dependencies resolved, ready to execute
    Ready,
    /// Currently executing
    Running,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed(String),
}

/// A build task in the DAG
#[derive(Debug, Clone)]
pub struct BuildTask {
    pub id: String,
    pub target: String,
    pub channel: String,
    pub dependencies: Vec<String>,
}

/// Task execution result
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub task_id: String,
    pub success: bool,
    pub duration_ms: u64,
    pub error: Option<String>,
}

/// Parallel executor for DAG-based builds
pub struct ParallelExecutor {
    /// Maximum concurrent tasks
    max_parallel: usize,
    /// Task graph with states
    tasks: HashMap<String, BuildTask>,
    /// Current state of each task
    states: Arc<Mutex<HashMap<String, TaskState>>>,
    /// Reverse dependency map (task -> tasks that depend on it)
    dependents: HashMap<String, Vec<String>>,
}

impl ParallelExecutor {
    /// Create new executor with given parallelism
    pub fn new(max_parallel: usize) -> Self {
        Self {
            max_parallel,
            tasks: HashMap::new(),
            states: Arc::new(Mutex::new(HashMap::new())),
            dependents: HashMap::new(),
        }
    }

    /// Add a task to the executor
    pub fn add_task(&mut self, task: BuildTask) {
        let task_id = task.id.clone();

        // Build reverse dependency map
        for dep in &task.dependencies {
            self.dependents
                .entry(dep.clone())
                .or_default()
                .push(task_id.clone());
        }

        self.tasks.insert(task_id, task);
    }

    /// Execute all tasks in parallel respecting dependencies
    pub async fn execute<F, Fut>(&self, task_fn: F) -> Result<Vec<TaskResult>>
    where
        F: Fn(BuildTask) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<TaskResult>> + Send,
    {
        use colored::Colorize;

        if self.tasks.is_empty() {
            return Ok(vec![]);
        }

        // Initialize states
        {
            let mut states = self.states.lock().await;
            for (id, task) in &self.tasks {
                let state = if task.dependencies.is_empty() {
                    TaskState::Ready
                } else {
                    TaskState::Pending
                };
                states.insert(id.clone(), state);
            }
        }

        let semaphore = Arc::new(Semaphore::new(self.max_parallel));
        let (tx, mut rx) = mpsc::channel::<TaskResult>(self.tasks.len());
        let states = Arc::clone(&self.states);
        let tasks = self.tasks.clone();
        let dependents = self.dependents.clone();

        let mut results = Vec::new();
        let mut completed_count = 0;
        let total_tasks = self.tasks.len();

        println!(
            "{}",
            format!(
                "🚀 Starting parallel build ({} tasks, {} workers)",
                total_tasks, self.max_parallel
            )
            .cyan()
        );

        // Spawn initial ready tasks
        let ready_tasks: Vec<_> = {
            let states = states.lock().await;
            states
                .iter()
                .filter(|(_, s)| **s == TaskState::Ready)
                .map(|(id, _)| id.clone())
                .collect()
        };

        for task_id in ready_tasks {
            let task = tasks.get(&task_id).unwrap().clone();
            let semaphore = Arc::clone(&semaphore);
            let tx = tx.clone();
            let states = Arc::clone(&states);
            let task_fn = task_fn.clone();

            tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();

                // Mark as running
                {
                    let mut states = states.lock().await;
                    states.insert(task.id.clone(), TaskState::Running);
                }

                let result = task_fn(task).await.unwrap_or_else(|e| TaskResult {
                    task_id: String::new(),
                    success: false,
                    duration_ms: 0,
                    error: Some(e.to_string()),
                });

                let _ = tx.send(result).await;
            });
        }

        // Process completions and spawn newly ready tasks
        while completed_count < total_tasks {
            if let Some(result) = rx.recv().await {
                completed_count += 1;

                let task_id = result.task_id.clone();
                let success = result.success;

                // Update state
                {
                    let mut states_guard = states.lock().await;
                    if success {
                        states_guard.insert(task_id.clone(), TaskState::Completed);
                    } else {
                        states_guard.insert(
                            task_id.clone(),
                            TaskState::Failed(result.error.clone().unwrap_or_default()),
                        );
                    }
                }

                // Print progress
                if success {
                    println!(
                        "{}",
                        format!(
                            "  ✅ [{}/{}] {} ({}ms)",
                            completed_count, total_tasks, task_id, result.duration_ms
                        )
                        .green()
                    );
                } else {
                    println!(
                        "{}",
                        format!(
                            "  ❌ [{}/{}] {} - {}",
                            completed_count,
                            total_tasks,
                            task_id,
                            result.error.as_deref().unwrap_or("unknown error")
                        )
                        .red()
                    );
                }

                results.push(result);

                // If successful, check dependents
                if success
                    && let Some(deps) = dependents.get(&task_id) {
                        for dep_id in deps {
                            let should_run = {
                                let states_guard = states.lock().await;

                                // Check if all dependencies are completed
                                let dep_task = tasks.get(dep_id).unwrap();
                                let all_deps_done = dep_task.dependencies.iter().all(|d| {
                                    matches!(
                                        states_guard.get(d),
                                        Some(TaskState::Completed)
                                    )
                                });

                                // Only run if pending and all deps done
                                all_deps_done
                                    && matches!(
                                        states_guard.get(dep_id),
                                        Some(TaskState::Pending)
                                    )
                            };

                            if should_run {
                                // Mark as ready then spawn
                                {
                                    let mut states_guard = states.lock().await;
                                    states_guard.insert(dep_id.clone(), TaskState::Ready);
                                }

                                let task = tasks.get(dep_id).unwrap().clone();
                                let semaphore = Arc::clone(&semaphore);
                                let tx = tx.clone();
                                let states = Arc::clone(&states);
                                let task_fn = task_fn.clone();

                                tokio::spawn(async move {
                                    let _permit = semaphore.acquire().await.unwrap();

                                    {
                                        let mut states = states.lock().await;
                                        states.insert(task.id.clone(), TaskState::Running);
                                    }

                                    let result =
                                        task_fn(task).await.unwrap_or_else(|e| TaskResult {
                                            task_id: String::new(),
                                            success: false,
                                            duration_ms: 0,
                                            error: Some(e.to_string()),
                                        });

                                    let _ = tx.send(result).await;
                                });
                            }
                        }
                    }
            }
        }

        // Summary
        let failed: Vec<_> = results.iter().filter(|r| !r.success).collect();
        let total_time: u64 = results.iter().map(|r| r.duration_ms).sum();

        println!();
        if failed.is_empty() {
            println!(
                "{}",
                format!("✅ All {} tasks completed successfully ({}ms total)", total_tasks, total_time)
                    .green()
                    .bold()
            );
        } else {
            println!(
                "{}",
                format!("❌ {} of {} tasks failed", failed.len(), total_tasks)
                    .red()
                    .bold()
            );
        }

        Ok(results)
    }
}

/// Get default parallelism (number of CPUs)
pub fn default_parallelism() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_parallelism() {
        let p = default_parallelism();
        assert!(p >= 1);
    }

    #[test]
    fn test_task_state() {
        assert_eq!(TaskState::Pending, TaskState::Pending);
        assert_ne!(TaskState::Pending, TaskState::Ready);
    }

    #[tokio::test]
    async fn test_executor_empty() {
        let executor = ParallelExecutor::new(4);
        let results = executor
            .execute(|_task| async { Ok(TaskResult {
                task_id: String::new(),
                success: true,
                duration_ms: 0,
                error: None,
            }) })
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_executor_single_task() {
        let mut executor = ParallelExecutor::new(4);
        executor.add_task(BuildTask {
            id: "task1".to_string(),
            target: "apps/web".to_string(),
            channel: "lts".to_string(),
            dependencies: vec![],
        });

        let results = executor
            .execute(|task| async move {
                Ok(TaskResult {
                    task_id: task.id,
                    success: true,
                    duration_ms: 10,
                    error: None,
                })
            })
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].success);
    }

    #[tokio::test]
    async fn test_executor_with_dependencies() {
        let mut executor = ParallelExecutor::new(4);

        // task2 depends on task1
        executor.add_task(BuildTask {
            id: "task1".to_string(),
            target: "libs/core".to_string(),
            channel: "lts".to_string(),
            dependencies: vec![],
        });
        executor.add_task(BuildTask {
            id: "task2".to_string(),
            target: "apps/web".to_string(),
            channel: "lts".to_string(),
            dependencies: vec!["task1".to_string()],
        });

        let execution_order = Arc::new(Mutex::new(Vec::new()));
        let order_clone = Arc::clone(&execution_order);

        let results = executor
            .execute(move |task| {
                let order = Arc::clone(&order_clone);
                async move {
                    order.lock().await.push(task.id.clone());
                    Ok(TaskResult {
                        task_id: task.id,
                        success: true,
                        duration_ms: 10,
                        error: None,
                    })
                }
            })
            .await
            .unwrap();

        assert_eq!(results.len(), 2);

        let order = execution_order.lock().await;
        // task1 must complete before task2
        let pos1 = order.iter().position(|x| x == "task1").unwrap();
        let pos2 = order.iter().position(|x| x == "task2").unwrap();
        assert!(pos1 < pos2);
    }
}
