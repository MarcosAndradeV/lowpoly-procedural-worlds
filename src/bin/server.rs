use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{Shutdown::Both, TcpListener, TcpStream},
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    thread,
    time::Duration,
};

use lowpoly_procedural_worlds::network::config_get_host_and_port;
use muxapp::muxui::{FromGssValue, Vector3, gss};

struct PlayerSession {
    id: u32,
    addr: String,
    pos: Vector3,
    yaw: f32,
    pitch: f32,
    mode: usize,
    stream: TcpStream,
}

struct SharedWorld {
    seed: usize,
    preset: usize,
    water: f32,
    time: usize,
    players: HashMap<u32, PlayerSession>,
}

static NEXT_PLAYER_ID: AtomicU32 = AtomicU32::new(1);

static WORLD: LazyLock<Mutex<SharedWorld>> = LazyLock::new(|| {
    Mutex::new(SharedWorld {
        seed: 4242,
        preset: 0,
        water: 14.0,
        time: 1,
        players: HashMap::new(),
    })
});

fn broadcast_packet(packet: &str) {
    if let Ok(mut world) = WORLD.lock() {
        let mut disconnected = Vec::new();
        for (&id, session) in world.players.iter_mut() {
            if session.stream.write_all(packet.as_bytes()).is_err() {
                disconnected.push(id);
            }
        }
        for id in disconnected {
            if let Some(removed) = world.players.remove(&id) {
                println!(
                    "[Server] Player #{} ({}) disconnected",
                    removed.id, removed.addr
                );
            }
        }
    }
}

fn broadcast_loop() {
    loop {
        thread::sleep(Duration::from_millis(40)); // 25 Hz state tick

        let snapshot = {
            let Ok(world) = WORLD.lock() else {
                continue;
            };
            if world.players.is_empty() {
                continue;
            }
            let mut msg = format!("type = \"state\"\ncount = {}\n", world.players.len());
            for (idx, session) in world.players.values().enumerate() {
                msg.push_str(&format!(
                    "id_{idx} = {}\npos_{idx} = {{x = {:.2}, y = {:.2}, z = {:.2}}}\nyaw_{idx} = {:.1}\npitch_{idx} = {:.1}\nmode_{idx} = {}\n",
                    session.id,
                    session.pos.x,
                    session.pos.y,
                    session.pos.z,
                    session.yaw,
                    session.pitch,
                    session.mode
                ));
            }
            msg.push_str("---\n");
            msg
        };

        broadcast_packet(&snapshot);
    }
}

fn handle_client_message(player_id: u32, source: &str) {
    let Ok(obj) = gss::parse_str(source) else {
        return;
    };
    let msg_type = String::from_gss(&obj, &["type"]).unwrap_or_default();

    match msg_type.as_str() {
        "pose" => {
            if let Ok(mut world) = WORLD.lock()
                && let Some(session) = world.players.get_mut(&player_id)
            {
                if let Some(pos) = Vector3::from_gss(&obj, &["pos"]) {
                    session.pos = pos;
                }
                if let Some(yaw) = f32::from_gss(&obj, &["yaw"]) {
                    session.yaw = yaw;
                }
                if let Some(pitch) = f32::from_gss(&obj, &["pitch"]) {
                    session.pitch = pitch;
                }
                if let Some(mode) = usize::from_gss(&obj, &["mode"]) {
                    session.mode = mode;
                }
            }
        }
        "world" => {
            let sync_pkt = {
                let Ok(mut world) = WORLD.lock() else {
                    return;
                };
                if let Some(seed) = i32::from_gss(&obj, &["seed"])
                    .map(|s| s as usize)
                    .or_else(|| usize::from_gss(&obj, &["seed"]))
                {
                    world.seed = seed;
                }
                if let Some(preset) = usize::from_gss(&obj, &["preset"]) {
                    world.preset = preset;
                }
                if let Some(water) = f32::from_gss(&obj, &["water"]) {
                    world.water = water;
                }
                if let Some(time) = usize::from_gss(&obj, &["time"]) {
                    world.time = time;
                }
                println!(
                    "[Server] Player #{} updated world: seed={}, preset={}, water={:.1}, time={}",
                    player_id, world.seed, world.preset, world.water, world.time
                );
                format!(
                    "type = \"sync\"\nseed = {}\npreset = {}\nwater = {:.2}\ntime = {}\n---\n",
                    world.seed, world.preset, world.water, world.time
                )
            };
            broadcast_packet(&sync_pkt);
        }
        "beacon" => {
            if let Some(pos) = Vector3::from_gss(&obj, &["pos"]) {
                println!(
                    "[Server] Player #{} summoned a Signal Beacon at ({:.1}, {:.1}, {:.1})",
                    player_id, pos.x, pos.y, pos.z
                );
                let pkt = format!(
                    "type = \"beacon\"\nowner = {}\npos = {{x = {:.2}, y = {:.2}, z = {:.2}}}\n---\n",
                    player_id, pos.x, pos.y, pos.z
                );
                broadcast_packet(&pkt);
            }
        }
        _ => {}
    }
}

fn handle_connection(mut stream: TcpStream) {
    let _ = stream.set_nodelay(true);
    let peer_addr = stream
        .peer_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let player_id = NEXT_PLAYER_ID.fetch_add(1, Ordering::Relaxed);
    // Offset each connecting player slightly around the starter overlook so they see each other immediately
    let angle = (player_id as f32) * 1.15;
    let initial_pos = Vector3::new(98.0 + angle.cos() * 12.0, 56.0, 185.0 + angle.sin() * 12.0);

    let Ok(write_clone) = stream.try_clone() else {
        return;
    };

    let welcome_msg = {
        let Ok(mut world) = WORLD.lock() else {
            return;
        };
        world.players.insert(
            player_id,
            PlayerSession {
                id: player_id,
                addr: peer_addr.clone(),
                pos: initial_pos,
                yaw: 180.0,
                pitch: -15.0,
                mode: 0,
                stream: write_clone,
            },
        );
        println!(
            "[Server] Player #{} connected from {} ({} active players)",
            player_id,
            peer_addr,
            world.players.len()
        );
        format!(
            "type = \"welcome\"\nid = {}\nseed = {}\npreset = {}\nwater = {:.2}\ntime = {}\npos = {{x = {:.2}, y = {:.2}, z = {:.2}}}\n---\n",
            player_id,
            world.seed,
            world.preset,
            world.water,
            world.time,
            initial_pos.x,
            initial_pos.y,
            initial_pos.z
        )
    };

    if stream.write_all(welcome_msg.as_bytes()).is_err() {
        return;
    }

    let mut recv_buf = String::with_capacity(2048);
    let mut chunk = [0u8; 1024];

    loop {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if let Ok(s) = std::str::from_utf8(&chunk[..n]) {
                    recv_buf.push_str(s);
                    while let Some(idx) = recv_buf.find("\n---\n") {
                        let pkt: String = recv_buf.drain(..idx).collect();
                        recv_buf.drain(..5);
                        handle_client_message(player_id, &pkt);
                    }
                }
            }
        }
    }

    if let Ok(mut world) = WORLD.lock() {
        world.players.remove(&player_id);
        println!(
            "[Server] Player #{} ({}) disconnected ({} active players)",
            player_id,
            peer_addr,
            world.players.len()
        );
    }
    let _ = stream.shutdown(Both);
}

fn main() -> std::io::Result<()> {
    let bind_addr = config_get_host_and_port();
    let listener = TcpListener::bind(&bind_addr)?;
    println!("======================================================");
    println!(" Low-Poly Procedural Worlds - Multiplayer GSS Server");
    println!(" Listening on {}", bind_addr);
    println!("======================================================");

    thread::spawn(broadcast_loop);

    thread::scope(|s| {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    s.spawn(|| handle_connection(stream));
                }
                Err(e) => {
                    eprintln!("[Server] Connection error: {e}");
                }
            }
        }
    });
    Ok(())
}
