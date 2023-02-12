use std::any::Any;
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
    responder: Arc<Mutex<Box<dyn ReloadResponder>>>
}

/// Internal private enum to track reload states
#[derive(PartialEq)]
pub enum ReloadState {
    None,
    Requested,
    Confirmed,
}

/// Check for reload changes
#[derive(PartialEq)]
pub enum ReloadResult {
    Continue,
    Reload
}

/// Trait to be implemented for custom reloader responses
pub trait ReloadResponder: Send + Sync {
    fn get_files(&self) -> &Vec<String>;
    fn build(&mut self);
    fn wait_for_completion(&mut self);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl Reloader {
    /// Create a new instance of a reload with the designated ReloadResponder
    pub fn create(responder: Arc<Mutex<Box<dyn ReloadResponder>>>) -> Self {
        Self {
            lock: Arc::new(Mutex::new(ReloadState::None)),
            responder
        }
    }

    /// Start watching for and invoking reload changes, this will spawn threads to watch files
    pub fn start(&self) {
        self.file_watcher_thread();
    }

    /// Call this each frame, if ReloadResult::Reload you must then clean up any data in preperation for a reload
    pub fn check_for_reload(&self) -> ReloadResult {
        let lock = self.lock.lock().unwrap();
        if *lock == ReloadState::Requested {
            ReloadResult::Reload
        }
        else {
            ReloadResult::Continue
        }
    }

    /// Once data is cleaned up and it is safe to proceed this functions must be called 
    pub fn complete_reload(&mut self) {
        println!("hotline_rs::reloader:: wait for completion");
        self.responder.lock().unwrap().wait_for_completion();

        let mut lock = self.lock.lock().unwrap();
        // signal it is safe to proceed and reload the new code
        *lock = ReloadState::Confirmed;
        drop(lock);
        println!("hotline_rs::reloader:: confirmed");
    }

    fn file_watcher_thread_check_mtime(responder: &Arc<Mutex<Box<dyn ReloadResponder>>>, cur_mtime: SystemTime) -> SystemTime {
        let responder = responder.lock().unwrap();
        let files = responder.get_files();
        for file in files {
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
        let mut cur_mtime = SystemTime::now();
        let lock = self.lock.clone();
        let responder = self.responder.clone();
        thread::spawn(move || {
            loop {
                
                let mtime = Self::file_watcher_thread_check_mtime(&responder, cur_mtime);

                if mtime > cur_mtime {
                    println!("hotline_rs::reloader: changes detected, building");

                    let mut responder = responder.lock().unwrap();
                    responder.build();

                    let mut a = lock.lock().unwrap();
                    println!("hotline_rs::reloader: reload requested");
                    *a = ReloadState::Requested;
                    drop(a);
        
                    cur_mtime = mtime;
                }

                std::thread::sleep(Duration::from_millis(16));
            }
        });
    }
}