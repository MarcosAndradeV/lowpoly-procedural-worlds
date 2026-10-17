# Chunk rendering

- STATUS: CLOSED
- PRIORITY: 90
- TAGS: rendering performance terrain

Replace the single monolithic camera-snapped terrain/prop/town grid with a spatial grid of independent world chunks (e.g. `16x16` or `32x32` cells per chunk), each owning its own retained `GpuMesh`:
- Only generate and upload newly entered chunks at the horizon perimeter as the camera moves, keeping existing chunks untouched in VRAM.
- Perform frustum culling per chunk before issuing `d3.draw_mesh(...)` calls.
- Eliminate full-grid rebuilds when crossing cell boundaries and allow much larger view distances with minimal CPU/GPU overhead.
