//! Parallel DAG Build Engine
//!
//! Executes build tasks in parallel respecting dependency order.
//! Uses tokio for async execution with configurable worker pool.

use anyhow::Result;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore, mpsc};

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

    /// Collect all transitive dependents of a given task
    fn collect_transitive_dependents(&self, task_id: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut queue = vec![task_id.to_string()];
        let mut visited = std::collections::HashSet::new();
        visited.insert(task_id.to_string());

        while let Some(current) = queue.pop() {
            if let Some(deps) = self.dependents.get(&current) {
                for dep in deps {
                    if visited.insert(dep.clone()) {
                        result.push(dep.clone());
                        queue.push(dep.clone());
                    }
                }
            }
        }
        result
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

        let multi = MultiProgress::new();
        let main_pb = multi.add(ProgressBar::new(self.tasks.len() as u64));
        main_pb.set_style(
            ProgressStyle::with_template(
                "{prefix:>12.cyan.bold} [{bar:40.cyan/blue}] {pos}/{len} {msg}",
            )
            .unwrap()
            .progress_chars("=>-"),
        );
        main_pb.set_prefix("Building");

        let semaphore = Arc::new(Semaphore::new(self.max_parallel));
        let (tx, mut rx) = mpsc::channel::<TaskResult>(self.tasks.len());
        let states = Arc::clone(&self.states);
        let tasks = self.tasks.clone();
        let dependents = self.dependents.clone();

        let mut results = Vec::new();
        let mut completed_count = 0;
        let total_tasks = self.tasks.len();

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
            let task = match tasks.get(&task_id) {
                Some(t) => t.clone(),
                None => continue,
            };
            let semaphore = Arc::clone(&semaphore);
            let tx = tx.clone();
            let states = Arc::clone(&states);
            let task_fn = task_fn.clone();
            let captured_id = task.id.clone();
            let multi_clone = multi.clone();

            tokio::spawn(async move {
                let _permit = semaphore
                    .acquire()
                    .await
                    .expect("semaphore closed unexpectedly");

                // Mark as running
                {
                    let mut states = states.lock().await;
                    states.insert(task.id.clone(), TaskState::Running);
                }

                let pb = multi_clone.add(ProgressBar::new_spinner());
                pb.set_style(
                    ProgressStyle::with_template("{spinner:.green} {msg:.dim}")
                        .unwrap()
                        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
                );
                pb.set_message(format!("Compiling {}", captured_id));
                pb.enable_steady_tick(std::time::Duration::from_millis(80));

                let result = task_fn(task).await.unwrap_or_else(|e| TaskResult {
                    task_id: captured_id,
                    success: false,
                    duration_ms: 0,
                    error: Some(e.to_string()),
                });

                pb.finish_and_clear();
                let _ = tx.send(result).await;
            });
        }

        // Process completions and spawn newly ready tasks
        while completed_count < total_tasks {
            if let Some(result) = rx.recv().await {
                completed_count += 1;
                main_pb.inc(1);

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

                // Print completion using PB log to avoid messing up bars
                if success {
                    main_pb.println(format!(
                        "{:>12} {} ({}ms)",
                        "Finished".green().bold(),
                        task_id,
                        result.duration_ms
                    ));
                } else {
                    main_pb.println(format!(
                        "{:>12} {} - {}",
                        "Failed".red().bold(),
                        task_id,
                        result.error.as_deref().unwrap_or("unknown error")
                    ));
                }

                results.push(result);

                // If failed, mark all transitive dependents as skipped
                if !success {
                    let skipped = self.collect_transitive_dependents(&task_id);
                    let mut states_guard = states.lock().await;
                    for skip_id in &skipped {
                        if matches!(states_guard.get(skip_id), Some(TaskState::Pending)) {
                            states_guard.insert(
                                skip_id.clone(),
                                TaskState::Failed(format!(
                                    "skipped: dependency '{}' failed",
                                    task_id
                                )),
                            );
                        }
                    }
                    drop(states_guard);

                    // Send skip results so the loop can terminate
                    for skip_id in skipped {
                        completed_count += 1;
                        main_pb.inc(1);
                        let skip_result = TaskResult {
                            task_id: skip_id.clone(),
                            success: false,
                            duration_ms: 0,
                            error: Some(format!("skipped: dependency '{}' failed", task_id)),
                        };
                        main_pb.println(format!(
                            "{:>12} {} - skipped (dependency failed)",
                            "Skipped".yellow().bold(),
                            skip_id
                        ));
                        results.push(skip_result);
                    }
                    continue;
                }

                // If successful, check dependents
                if let Some(deps) = dependents.get(&task_id) {
                    for dep_id in deps {
                        let should_run = {
                            let states_guard = states.lock().await;

                            let Some(dep_task) = tasks.get(dep_id) else {
                                continue;
                            };
                            let all_deps_done = dep_task
                                .dependencies
                                .iter()
                                .all(|d| matches!(states_guard.get(d), Some(TaskState::Completed)));

                            all_deps_done
                                && matches!(states_guard.get(dep_id), Some(TaskState::Pending))
                        };

                        if should_run {
                            {
                                let mut states_guard = states.lock().await;
                                states_guard.insert(dep_id.clone(), TaskState::Ready);
                            }

                            let task = match tasks.get(dep_id) {
                                Some(t) => t.clone(),
                                None => continue,
                            };
                            let semaphore = Arc::clone(&semaphore);
                            let tx = tx.clone();
                            let states = Arc::clone(&states);
                            let task_fn = task_fn.clone();
                            let captured_id = task.id.clone();
                            let multi_clone = multi.clone();

                            tokio::spawn(async move {
                                let _permit = semaphore
                                    .acquire()
                                    .await
                                    .expect("semaphore closed unexpectedly");

                                {
                                    let mut states = states.lock().await;
                                    states.insert(task.id.clone(), TaskState::Running);
                                }

                                let pb = multi_clone.add(ProgressBar::new_spinner());
                                pb.set_style(
                                    ProgressStyle::with_template("{spinner:.green} {msg:.dim}")
                                        .unwrap()
                                        .tick_strings(&[
                                            "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏",
                                        ]),
                                );
                                pb.set_message(format!("Compiling {}", captured_id));
                                pb.enable_steady_tick(std::time::Duration::from_millis(80));

                                let result = task_fn(task).await.unwrap_or_else(|e| TaskResult {
                                    task_id: captured_id,
                                    success: false,
                                    duration_ms: 0,
                                    error: Some(e.to_string()),
                                });

                                pb.finish_and_clear();
                                let _ = tx.send(result).await;
                            });
                        }
                    }
                }
            }
        }

        main_pb.finish_and_clear();

        // Summary
        let failed: Vec<_> = results.iter().filter(|r| !r.success).collect();
        let total_time: u64 = results.iter().map(|r| r.duration_ms).sum();

        if failed.is_empty() {
            println!(
                "{:>12} All {} tasks in {}ms",
                "Success".green().bold(),
                total_tasks,
                total_time
            );
        } else {
            eprintln!(
                "{:>12} {} of {} tasks failed",
                "Error".red().bold(),
                failed.len(),
                total_tasks
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
            .execute(|_task| async {
                Ok(TaskResult {
                    task_id: String::new(),
                    success: true,
                    duration_ms: 0,
                    error: None,
                })
            })
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
