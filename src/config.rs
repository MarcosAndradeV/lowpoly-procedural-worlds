use std::sync::{LazyLock, Mutex};

use muxapp::muxui::{gss, log_error, log_info};

const CONFIG_FILE_PATH: &str = "./config.gss";

static CONFIG: LazyLock<Mutex<gss::Object>> = LazyLock::new(|| Mutex::new(load_config()));

fn load_config() -> gss::Object {
    match gss::load_gss_from_file(CONFIG_FILE_PATH) {
        Ok(config) => {
            log_info!("CONFIG: Loaded `{CONFIG_FILE_PATH}`");
            config
        }
        Err(err) => {
            log_error!("CONFIG: Cannot load `{CONFIG_FILE_PATH}`: {err}");
            log_info!("CONFIG: Using built-in defaults");
            gss::Object::new()
        }
    }
}

fn with_config<R>(f: impl FnOnce(&mut gss::Object) -> R) -> R {
    let mut guard = CONFIG.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

pub fn reload() {
    let fresh = load_config();
    with_config(|config| *config = fresh);
}

macro_rules! settings {
    ($( $(#[$meta:meta])* $name:ident : $ty:ty = [$($path:literal),+] => $default:expr; )*) => {
        $(
            $(#[$meta])*
            pub fn $name() -> $ty {
                with_config(|config| config.get_or(&[$($path),+], $default))
            }
        )*
    };
}

settings! {
    /// Target frames per second.
    get_fps: u32 = ["performance", "fps"] => 60;
    /// Server host name or IP.
    get_host: String = ["network", "host"] => "127.0.0.1".to_string();
    /// Server port.
    get_port: String = ["network", "port"] => "8080".to_string();
}

pub fn get_host_and_port() -> String {
    format!("{}:{}", get_host(), get_port())
}
