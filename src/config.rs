use muxapp::muxui::{gss, log_error, log_info};

pub fn get_fps() -> u32 {
    match gss::load_gss_from_file("./config.gss") {
        Ok(ok) => ok.get_or(&["performace", "fps"], 60),
        Err(err) => {
            log_error!("CONFIG: Cannot load `config.gss` {err}");
            log_info!("CONFIG: Using default fps: `60`");
            60
        }
    }
}

pub fn get_host_and_port() -> String {
    match gss::load_gss_from_file("./config.gss") {
        Ok(ok) => {
            let host = ok.get_or(&["network", "host"], "127.0.0.1".to_string());
            let port = ok.get_or(&["network", "port"], "8080".to_string());
            format!("{}:{}", host, port)
        }
        Err(err) => {
            log_error!("CONFIG: Cannot load `config.gss` {err}");
            log_info!("CONFIG: Using default address: `127.0.0.1:8080`");
            "127.0.0.1:8080".to_string()
        }
    }
}
