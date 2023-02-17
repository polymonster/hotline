use std::process::ExitStatus;
use std::time::SystemTime;
use std::time::Duration;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

/// Basic Reloader which can check timestamps on files and then callback functions supplied by the reload responder
pub struct Reloader {
    /// Hash map storing files grouped by type (pmfx, code) and then keep a vector of files
    /// and timestamps for quick checking at run time.
    lock: Arc<Mutex<ReloadState>>,
    /// You can implement your own `ReloadResponder` trait to get callback functions to trigger a build
    responder: Arc<Mutex<Box<dyn ReloadResponder>>>
}

/// Query reload status with a responder:
/// if 
#[derive(PartialEq, Clone, Copy)]
pub enum ReloadState {
    /// No action needs taking
    None,
    /// There is a reload available, get into a stable state and call `complete_reload` to complete
    Available,
}

/// Trait to be implemented for custom reloader responses
pub trait ReloadResponder: Send + Sync {
    /// Add a file which is tracked and the time stamp compared for changes
    fn add_file(&mut self, path: &str);
    /// Returns a vector of files which are currently being tracked
    fn get_files(&self) -> Vec<String>;
    /// Retuns the current modified time of the built resource
    fn get_last_mtime(&self) -> SystemTime;
    /// Called when a tracked file is modified more recently than get_base_mtime
    fn build(&mut self) -> ExitStatus;
}

impl Reloader {
    /// Create a new instance of a reload with the designated ReloadResponder and start waiting for file changes
    pub fn create(responder: Box<dyn ReloadResponder>) -> Self {
        Self {
            lock: Arc::new(Mutex::new(ReloadState::None)),
            responder: Arc::new(Mutex::new(responder)),
        }.start()
    }

    /// Add files to check in a thread safe manner
    pub fn add_file(&mut self, path: &str) {
        let mut responder = self.responder.lock().unwrap();
        responder.add_file(path);
    }

    /// Start watching for and invoking reload changes, this will spawn threads to watch files
    pub fn start(self) -> Self {
        self.file_watcher_thread();
        self
    }

    /// Call this each frame, if ReloadResult::Reload you must then clean up any data in preperation for a reload
    pub fn check_for_reload(&self) -> ReloadState {
        let lock = self.lock.lock().unwrap();
        *lock
    }

    /// Once data is cleaned up and it is safe to proceed this functions must be called 
    pub fn complete_reload(&mut self) {
        let mut lock = self.lock.lock().unwrap();
        // signal it is safe to proceed and reload the new code
        *lock = ReloadState::None;
        drop(lock);
        println!("hotline_rs::reloader: reload complete");
    }

    /// Returns the latest timestamp of all the files tracked by the reloader
    fn file_watcher_thread_check_mtime(responder: &Arc<Mutex<Box<dyn ReloadResponder>>>, cur_mtime: SystemTime) -> SystemTime {
        let responder = responder.lock().unwrap();
        let files = responder.get_files();
        for file in &files {
            let filepath = super::get_data_path(file);
            let meta = std::fs::metadata(&filepath);
            if meta.is_ok() {
                let mtime = std::fs::metadata(&filepath).unwrap().modified().unwrap();
                if mtime > cur_mtime {
                    return mtime;
                }
            }
            else {
                print!("hotline_rs::reloader: {filepath} not found!")
            }
        };
        cur_mtime
    }

    /// Background thread will watch for changed filestamps among the registered files from the responder
    fn file_watcher_thread(&self) {
        let lock = self.lock.clone();
        let mut cur_mtime = SystemTime::now();
        let mut first_time_check = true;
        let responder = self.responder.clone();
        thread::Builder::new().name("hotline_rs::reloader::file_watcher_thread".to_string()).spawn(move || {
            loop {
                // check base mtime of the output lib, it might be old / stale when we run with a fresh client
                if first_time_check {
                    cur_mtime = responder.lock().unwrap().get_last_mtime();
                    first_time_check = false;
                }

                let mtime = Self::file_watcher_thread_check_mtime(&responder, cur_mtime);
                if mtime > cur_mtime {
                    println!("hotline_rs::reloader: changes detected, building");
                    let mut responder = responder.lock().unwrap();
                    if responder.build().success() {
                        let mut a = lock.lock().unwrap();
                        println!("hotline_rs::reloader: build success, reload available");
                        *a = ReloadState::Available;
                        drop(a);
                    }
                    else {
                        println!("hotline_rs::reloader: build failed");
                    }
                    cur_mtime = mtime;
                }
                std::thread::sleep(Duration::from_millis(16));
            }
        }).unwrap();
    }
}