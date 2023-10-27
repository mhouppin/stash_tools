use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::engine::UciEngine;

pub struct TaskQueue {
    workload: VecDeque<String>,
    response: VecDeque<String>,
    workload_finished: bool,
    active_workers: usize,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self {
            workload: VecDeque::new(),
            response: VecDeque::new(),
            workload_finished: false,
            active_workers: 0,
        }
    }

    pub fn add_workload(&mut self, fen: String) {
        self.workload.push_back(fen);
    }

    pub fn query_workload(&mut self) -> Option<String> {
        self.workload.pop_front()
    }

    pub fn stop_workload(&mut self) {
        self.workload_finished = true;
    }

    pub fn is_workload_finished(&self) -> bool {
        self.workload_finished
    }

    pub fn add_response(&mut self, scored_fen: String) {
        self.response.push_back(scored_fen)
    }

    pub fn query_response(&mut self) -> Option<String> {
        self.response.pop_front()
    }

    pub fn add_worker(&mut self) {
        self.active_workers += 1;
    }

    pub fn remove_worker(&mut self) {
        self.active_workers -= 1;
    }

    pub fn no_active_workers(&mut self) -> bool {
        self.active_workers == 0
    }
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TaskWorker {
    engine: UciEngine,
    queue: Arc<Mutex<TaskQueue>>,
}

impl TaskWorker {
    pub fn new(queue: &Arc<Mutex<TaskQueue>>, engine_path: &str, config: &Vec<String>) -> Self {
        let mut worker = Self {
            engine: UciEngine::try_new(engine_path).unwrap(),
            queue: queue.clone(),
        };

        worker.engine.init_protocol(config).unwrap();
        worker.queue.lock().unwrap().add_worker();
        worker
    }

    pub fn engine_mut(&mut self) -> &mut UciEngine {
        &mut self.engine
    }

    pub fn query_workload(&mut self) -> Option<String> {
        loop {
            let mut queue = self.queue.lock().unwrap();

            if let Some(fen) = queue.query_workload() {
                return Some(fen);
            }

            if queue.is_workload_finished() {
                break;
            }

            drop(queue);
            thread::sleep(Duration::from_micros(10));
        }

        None
    }

    pub fn fill_response(&mut self, scored_fen: String) {
        let mut queue = self.queue.lock().unwrap();

        queue.add_response(scored_fen);
    }

    pub fn remove_worker(&mut self) {
        let mut queue = self.queue.lock().unwrap();

        queue.remove_worker();
    }
}

pub struct TaskClient {
    queue: Arc<Mutex<TaskQueue>>,
}

impl TaskClient {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(TaskQueue::new())),
        }
    }

    pub fn queue_ref(&self) -> &Arc<Mutex<TaskQueue>> {
        &self.queue
    }

    pub fn add_workload(&mut self, fen: String) {
        let mut queue = self.queue.lock().unwrap();

        queue.add_workload(fen);
    }

    pub fn stop_workload(&mut self) {
        let mut queue = self.queue.lock().unwrap();

        queue.stop_workload();
    }

    pub fn query_response(&mut self, retry: bool) -> Option<String> {
        loop {
            let mut queue = self.queue.lock().unwrap();

            if let Some(scored_fen) = queue.query_response() {
                return Some(scored_fen);
            }

            if queue.no_active_workers() || !retry {
                break;
            }

            drop(queue);
            thread::sleep(Duration::from_micros(10));
        }

        None
    }
}

impl Default for TaskClient {
    fn default() -> Self {
        Self::new()
    }
}
