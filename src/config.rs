use muxapp::muxui::gss::lazy_gss;

lazy_gss! {
    pub static CONFIG = "./config.gss";

    /// Target frames per second.
    pub get_fps() -> u32 = ["performance", "fps"] or_else 60;
    /// Server host name or IP.
    pub get_host() -> String = ["network", "host"] or_else "127.0.0.1".to_string();
    /// Server port.
    pub get_port() -> String = ["network", "port"] or_else "8080".to_string();
}

pub fn get_host_and_port() -> String {
    format!("{}:{}", get_host(), get_port())
}
