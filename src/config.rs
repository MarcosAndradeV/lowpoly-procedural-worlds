use std::sync::{
    LazyLock, Mutex,
    atomic::{AtomicBool, Ordering},
};

use muxapp::muxui::{gss, log_error, log_info};

static CONFIG_IS_LOADED: AtomicBool = AtomicBool::new(false);
static CONFIG_CACHE: LazyLock<Mutex<gss::Object>> =
    LazyLock::new(|| Mutex::new(gss::Object::new()));

const CONFIG_FILE_PATH: &'static str = "./config.gss";
const DEFAULT_FPS: u32 = 60;
macro_rules! DEFAULT_HOST {
    () => {
        "127.0.0.1"
    };
}
macro_rules! DEFAULT_PORT {
    () => {
        "8080"
    };
}
const DEFAULT_ADDRESS: &str = concat!(DEFAULT_HOST!(), ":", DEFAULT_PORT!());

pub fn get_fps() -> u32 {
    let mut config = CONFIG_CACHE.lock().unwrap();
    if CONFIG_IS_LOADED.load(Ordering::Relaxed) {
        config.get_or(&["performace", "fps"], DEFAULT_FPS)
    } else {
        match gss::load_gss_from_file(CONFIG_FILE_PATH) {
            Ok(ok) => {
                CONFIG_IS_LOADED.store(true, Ordering::Relaxed);
                *config = ok;
                config.get_or(&["performace", "fps"], DEFAULT_FPS)
            }
            Err(err) => {
                log_error!("CONFIG: Cannot load `config.gss` {err}");
                log_info!("CONFIG: Using default fps: `{DEFAULT_FPS}`");
                DEFAULT_FPS
            }
        }
    }
}

pub fn get_host_and_port() -> String {
    let mut config = CONFIG_CACHE.lock().unwrap();
    if CONFIG_IS_LOADED.load(Ordering::Relaxed) {
        let host = config.get_or(&["network", "host"], DEFAULT_HOST!().to_string());
        let port = config.get_or(&["network", "port"], DEFAULT_PORT!().to_string());
        format!("{}:{}", host, port)
    } else {
        match gss::load_gss_from_file(CONFIG_FILE_PATH) {
            Ok(ok) => {
                CONFIG_IS_LOADED.store(true, Ordering::Relaxed);
                *config = ok;
                let host = config.get_or(&["network", "host"], DEFAULT_HOST!().to_string());
                let port = config.get_or(&["network", "port"], DEFAULT_PORT!().to_string());
                format!("{}:{}", host, port)
            }
            Err(err) => {
                log_error!("CONFIG: Cannot load `config.gss` {err}");
                log_info!("CONFIG: Using default address: `{DEFAULT_ADDRESS}`");
                DEFAULT_ADDRESS.to_string()
            }
        }
    }
}
