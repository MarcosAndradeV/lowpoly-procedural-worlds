# lppw-net

Multiplayer networking protocol, GSS state synchronization, beacon tracking, and 3D humanoid avatar rendering.

## Features

- **GSS State Sync**: High-efficiency delta state packet parsing supporting signed floats, player orientations, camera modes, and walk cycle animation.
- **Wavefront OBJ Avatar**: Headless or rendered humanoid adventurer avatar loaded from `human_low.obj` with procedural player tunic colors.
- **Signal Beacons**: Timed expanding 3D aura rings and pillars broadcasted across the multiplayer server.
- **Non-blocking TCP**: Ultra-low latency network client with automatic reconnection and non-blocking I/O.
