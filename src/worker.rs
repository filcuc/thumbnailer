use std::sync::{Arc, Mutex, Condvar};
use std::path::{Path, PathBuf};
use std::thread::{Thread, JoinHandle};
use crate::thumbnailer::ThumbSize;
use image::imageops::thumbnail;
use crate::generate_thumbnail;
use log::{debug};
use std::collections::VecDeque;

struct GenerateData {
    source: PathBuf,
    sizes: Vec<ThumbSize>,
    destination: PathBuf
}

enum Message {
    Exit,
    Generate(GenerateData)
}

pub struct Worker {
    queue: Arc<Mutex<VecDeque<Message>>>,
    cond: Arc<Condvar>,
    workers: Vec<JoinHandle<()>>,
}

impl Worker {
    pub fn new(num_workers: u32) -> Worker {
        let cond = Arc::new(Condvar::new());
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let mut workers: Vec<JoinHandle<_>> = Vec::new();
        for _i in 0..num_workers {
            let cond = cond.clone();
            let queue = queue.clone();
            workers.push(std::thread::spawn(move|| Worker::work(cond, queue)));
        }
        Worker {
            cond,
            workers,
            queue
        }
    }

    pub fn push(&self, source: PathBuf,
                sizes: Vec<ThumbSize>,
                destination: PathBuf) {
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(Message::Generate(GenerateData{ source, sizes, destination}));
        self.cond.notify_all();
    }

    fn work(cond: Arc<Condvar>, queue: Arc<Mutex<VecDeque<Message>>>) {
        loop {
            let mut m: Option<Message> = None;
            {
                {
                    let mut guard = queue.lock().unwrap();
                    m = guard.pop_front();
                    if m.is_none() {
                        guard = cond.wait(guard).unwrap();
                    }
                }
                if let Some(m) = m {
                    match m {
                        Message::Exit => { return; },
                        Message::Generate(data) => {
                            generate_thumbnail(data.source, data.sizes, &data.destination);
                        },
                    }
                }
            }
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        {
            let mut guard = self.queue.lock().unwrap();
            for _w in &self.workers {
                guard.push_back(Message::Exit);
            }
            self.cond.notify_all();
        }

        while !self.workers.is_empty() {
            self.workers.pop().unwrap().join();
        }
    }
}