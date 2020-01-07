/**
    This file is part of Thumbnailer.

    Thumbnailer is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License.

    Thumbnailer is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with Thumbnailer.  If not, see <http://www.gnu.org/licenses/>.
*/
use log::error;
use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

type Action = Box<dyn FnOnce() -> () + Send>;

pub enum Message {
    Action(Action),
    Exit()
}

pub struct Worker {
    queue: Arc<Mutex<VecDeque<Message>>>,
    cond: Arc<Condvar>,
    workers: Vec<JoinHandle<()>>,
}

impl Worker {
    pub fn new(num_workers: u32) -> Worker {
        let cond = Arc::new(Condvar::new());
        let queue= Arc::new(Mutex::new(VecDeque::new()));
        let mut workers: Vec<JoinHandle<_>> = Vec::new();
        for _i in 0..num_workers {
            let cond = cond.clone();
            let queue = queue.clone();
            workers.push(std::thread::spawn(move || Worker::work(cond, queue)));
        }
        Worker {
            cond,
            workers,
            queue,
        }
    }

    pub fn push(&self, action: Action) {
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(crate::worker::Message::Action(action));
        self.cond.notify_all();
    }

    fn work(cond: Arc<Condvar>, queue: Arc<Mutex<VecDeque<Message>>>) {
        loop {
            let mut guard = queue.lock().unwrap();
            while guard.is_empty() {
                guard = cond.wait(guard).unwrap();
            }

            let action = guard.pop_front();
            std::mem::drop(guard);

            match action {
                Some(Message::Exit()) => return,
                Some(Message::Action(a)) => a(),
                _ => {}
            }
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        {
            let mut guard = self.queue.lock().unwrap();
            for _w in &self.workers {
                guard.push_back(Message::Exit());
            }
            self.cond.notify_all();
        }

        while !self.workers.is_empty() {
            if let Err(_) = self.workers.pop().unwrap().join() {
                error!("Could not join worker thread");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::worker::Worker;
    use crate::worker::Message::Action;
    use std::sync::{Arc, Mutex};
    use std::ops::Deref;

    #[test]
    fn test_creation() {
        let worker = Worker::new(1);
        std::mem::drop(worker);
    }

    #[test]
    fn test_push() {
        let worker = Worker::new(1);
        let executed = Arc::new(Mutex::new(false));
        let e = executed.clone();
        worker.push(Box::new(move||{
            let mut value = e.lock().unwrap();
            *value = true;
        }));
        std::mem::drop(worker);
        assert!(*executed.lock().unwrap());
    }
}

